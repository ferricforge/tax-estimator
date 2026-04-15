use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, LitStr, parse_macro_input};

#[proc_macro_derive(Entity, attributes(entity))]
pub fn derive_entity(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

struct FieldMeta {
    ident: syn::Ident,
    ty: syn::Type,
    skip: bool,
    pk: bool,
    encode_with: Option<syn::Path>,
    decode_with: Option<syn::Path>,
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let ident = &input.ident;

    // ── container attributes ──────────────────────────────────────────
    let mut table: Option<String> = None;
    for attr in &input.attrs {
        if attr.path().is_ident("entity") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("table") {
                    let v: LitStr = meta.value()?.parse()?;
                    table = Some(v.value());
                    Ok(())
                } else {
                    Err(meta.error("unknown `entity` container attribute"))
                }
            })?;
        }
    }
    let table = table.unwrap_or_else(|| to_snake_plural(&ident.to_string()));

    // ── fields ────────────────────────────────────────────────────────
    let named = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(n) => &n.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "`Entity` requires named fields",
                ));
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                ident,
                "`Entity` can only be derived for structs",
            ));
        }
    };

    let mut all_fields: Vec<FieldMeta> = Vec::new();

    for f in named {
        let mut skip = false;
        let mut pk = false;
        let mut encode_with: Option<syn::Path> = None;
        let mut decode_with: Option<syn::Path> = None;

        for attr in &f.attrs {
            if attr.path().is_ident("entity") {
                attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("skip") {
                        skip = true;
                        Ok(())
                    } else if meta.path.is_ident("pk") {
                        pk = true;
                        Ok(())
                    } else if meta.path.is_ident("encode_with") {
                        let v: LitStr = meta.value()?.parse()?;
                        encode_with = Some(v.parse()?);
                        Ok(())
                    } else if meta.path.is_ident("decode_with") {
                        let v: LitStr = meta.value()?.parse()?;
                        decode_with = Some(v.parse()?);
                        Ok(())
                    } else {
                        Err(meta.error("unknown `entity` field attribute"))
                    }
                })?;
            }
        }

        if skip && pk {
            return Err(syn::Error::new_spanned(
                f.ident.as_ref().unwrap(),
                "a field cannot be both `skip` and `pk`",
            ));
        }

        all_fields.push(FieldMeta {
            ident: f.ident.clone().unwrap(),
            ty: f.ty.clone(),
            skip,
            pk,
            encode_with,
            decode_with,
        });
    }

    let included: Vec<&FieldMeta> = all_fields.iter().filter(|f| !f.skip).collect();
    let skipped: Vec<&FieldMeta> = all_fields.iter().filter(|f| f.skip).collect();
    let pk_fields: Vec<&FieldMeta> = included.iter().filter(|f| f.pk).copied().collect();
    let has_pk = !pk_fields.is_empty();

    if included.is_empty() {
        return Err(syn::Error::new_spanned(
            ident,
            "`Entity` needs at least one non-skipped field",
        ));
    }

    // ── SQL strings ───────────────────────────────────────────────────
    let columns: Vec<String> = included.iter().map(|f| f.ident.to_string()).collect();
    let col_list = columns.join(", ");
    let placeholders = vec!["?"; columns.len()].join(", ");
    let insert_sql = format!("INSERT INTO {table} ({col_list}) VALUES ({placeholders})");
    let select_all_sql = format!("SELECT {col_list} FROM {table}");

    // ── INSERT bind calls ─────────────────────────────────────────────
    let insert_binds: Vec<proc_macro2::TokenStream> = included
        .iter()
        .map(|f| {
            let name = &f.ident;
            match &f.encode_with {
                Some(path) => quote! { .bind(#path(&self.#name)) },
                None => quote! { .bind(&self.#name) },
            }
        })
        .collect();

    // Suppress dead-code warnings for skipped fields
    let skip_reads: Vec<proc_macro2::TokenStream> = skipped
        .iter()
        .map(|f| {
            let name = &f.ident;
            quote! { let _ = &self.#name; }
        })
        .collect();

    // ── Row → Self mapping (used by from_row) ─────────────────────────
    let row_mappings: Vec<proc_macro2::TokenStream> = all_fields
        .iter()
        .map(|f| {
            let name = &f.ident;
            if f.skip {
                quote! { #name: ::std::default::Default::default() }
            } else {
                let col = name.to_string();
                match &f.decode_with {
                    Some(path) => quote! { #name: #path(&__entity_row, #col)? },
                    None => quote! { #name: ::sqlx::Row::try_get(&__entity_row, #col)? },
                }
            }
        })
        .collect();

    // ── Always-generated block ────────────────────────────────────────
    let mut output = quote! {
        impl #ident {
            pub const TABLE: &'static str = #table;
            pub const INSERT_SQL: &'static str = #insert_sql;
            pub const SELECT_ALL_SQL: &'static str = #select_all_sql;

            fn from_row(
                __entity_row: ::sqlx::sqlite::SqliteRow,
            ) -> ::std::result::Result<Self, ::sqlx::Error> {
                ::std::result::Result::Ok(Self {
                    #(#row_mappings,)*
                })
            }

            pub async fn insert<'e, E>(
                &self,
                executor: E,
            ) -> ::std::result::Result<
                ::sqlx::sqlite::SqliteQueryResult,
                ::sqlx::Error,
            >
            where
                E: ::sqlx::Executor<'e, Database = ::sqlx::Sqlite>,
            {
                #(#skip_reads)*
                ::sqlx::query(Self::INSERT_SQL)
                    #(#insert_binds)*
                    .execute(executor)
                    .await
            }

            pub async fn list<'e, E>(
                executor: E,
            ) -> ::std::result::Result<
                ::std::vec::Vec<Self>,
                ::sqlx::Error,
            >
            where
                E: ::sqlx::Executor<'e, Database = ::sqlx::Sqlite>,
            {
                let rows = ::sqlx::query(Self::SELECT_ALL_SQL)
                    .fetch_all(executor)
                    .await?;
                rows.into_iter()
                    .map(Self::from_row)
                    .collect()
            }
        }
    };

    // ── PK-dependent block ────────────────────────────────────────────
    if has_pk {
        let pk_where = pk_fields
            .iter()
            .map(|f| format!("{} = ?", f.ident))
            .collect::<Vec<_>>()
            .join(" AND ");
        let get_sql = format!("{select_all_sql} WHERE {pk_where}");
        let delete_sql = format!("DELETE FROM {table} WHERE {pk_where}");

        // fn find(executor, &pk1, &pk2, …)
        let pk_params: Vec<proc_macro2::TokenStream> = pk_fields
            .iter()
            .map(|f| {
                let name = &f.ident;
                let ty = &f.ty;
                quote! { #name: &#ty }
            })
            .collect();

        // .bind(pk) for find / delete_by_pk (params are already references)
        let pk_binds: Vec<proc_macro2::TokenStream> = pk_fields
            .iter()
            .map(|f| {
                let name = &f.ident;
                match &f.encode_with {
                    Some(path) => quote! { .bind(#path(#name)) },
                    None => quote! { .bind(#name) },
                }
            })
            .collect();

        // .bind(&self.pk) for delete(&self, …)
        let self_pk_binds: Vec<proc_macro2::TokenStream> = pk_fields
            .iter()
            .map(|f| {
                let name = &f.ident;
                match &f.encode_with {
                    Some(path) => quote! { .bind(#path(&self.#name)) },
                    None => quote! { .bind(&self.#name) },
                }
            })
            .collect();

        output.extend(quote! {
            impl #ident {
                pub const GET_SQL: &'static str = #get_sql;
                pub const DELETE_SQL: &'static str = #delete_sql;

                pub async fn find<'e, E>(
                    executor: E,
                    #(#pk_params,)*
                ) -> ::std::result::Result<
                    ::std::option::Option<Self>,
                    ::sqlx::Error,
                >
                where
                    E: ::sqlx::Executor<'e, Database = ::sqlx::Sqlite>,
                {
                    let maybe_row = ::sqlx::query(Self::GET_SQL)
                        #(#pk_binds)*
                        .fetch_optional(executor)
                        .await?;
                    maybe_row.map(Self::from_row).transpose()
                }

                pub async fn delete_by_pk<'e, E>(
                    executor: E,
                    #(#pk_params,)*
                ) -> ::std::result::Result<
                    ::sqlx::sqlite::SqliteQueryResult,
                    ::sqlx::Error,
                >
                where
                    E: ::sqlx::Executor<'e, Database = ::sqlx::Sqlite>,
                {
                    ::sqlx::query(Self::DELETE_SQL)
                        #(#pk_binds)*
                        .execute(executor)
                        .await
                }

                pub async fn delete<'e, E>(
                    &self,
                    executor: E,
                ) -> ::std::result::Result<
                    ::sqlx::sqlite::SqliteQueryResult,
                    ::sqlx::Error,
                >
                where
                    E: ::sqlx::Executor<'e, Database = ::sqlx::Sqlite>,
                {
                    ::sqlx::query(Self::DELETE_SQL)
                        #(#self_pk_binds)*
                        .execute(executor)
                        .await
                }
            }
        });
    }

    Ok(output)
}

fn to_snake_plural(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if ch.is_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out.push('s');
    out
}

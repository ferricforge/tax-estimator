#!/usr/bin/env python3
"""Generate the checked-in tax-ui field-instruction catalog.

This script intentionally keeps the final tooltip text curated and compact,
while validating that each configured source anchor still exists in the IRS
PDFs stored under `docs/`. If the anchors drift, the script fails so the
catalog can be reviewed against the updated forms.
"""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from pypdf import PdfReader


REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_PATH = REPO_ROOT / "tax-ui" / "resources" / "field_instructions.toml"


@dataclass(frozen=True)
class Source:
    file: str
    page: int
    section: str
    anchor: str


@dataclass(frozen=True)
class Field:
    key: str
    label: str
    summary: str
    detail: str | None = None
    sources: tuple[Source, ...] = ()


@dataclass(frozen=True)
class Year:
    year: int
    fields: tuple[Field, ...]


@dataclass(frozen=True)
class Form:
    id: str
    title: str
    years: tuple[Year, ...]


def forms() -> tuple[Form, ...]:
    return (
        Form(
            id="1040-es",
            title="Form 1040-ES",
            years=(
                Year(
                    year=2025,
                    fields=(
                        Field(
                            key="expected_agi",
                            label="Expected AGI",
                            summary="Adjusted gross income you expect in 2025.",
                            detail=(
                                "When figuring the adjusted gross income you expect in 2025, "
                                "consider the current-year items called out in the form "
                                "instructions. If you are self-employed, subtract the "
                                "deductible part of self-employment tax by using the 2025 "
                                "Self-Employment Tax and Deduction Worksheet."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 5, "Line 1", "Line 1. Adjusted gross income."),
                            ),
                        ),
                        Field(
                            key="expected_deduction",
                            label="Expected deduction",
                            summary="Deductions.",
                            detail=(
                                "If you plan to itemize deductions, enter the estimated total "
                                "of your itemized deductions. If you do not plan to itemize "
                                "deductions, enter your standard deduction."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 8, "Line 2a", "2a Deductions"),
                            ),
                        ),
                        Field(
                            key="expected_qbi_deduction",
                            label="QBI deduction",
                            summary=(
                                "If you can take the qualified business income deduction, "
                                "enter the estimated amount of the deduction."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 8, "Line 2b", "qualified business income deduction"),
                            ),
                        ),
                        Field(
                            key="expected_amt",
                            label="AMT",
                            summary="Alternative minimum tax from Form 6251.",
                            sources=(
                                Source("docs/f1040es_2025.pdf", 8, "Line 5", "Alternative minimum tax from Form 6251"),
                            ),
                        ),
                        Field(
                            key="expected_credits",
                            label="Credits",
                            summary="Credits. Do not include any income tax withholding on this line.",
                            detail=(
                                "See the 2024 Form 1040 or 1040-SR, line 19, Schedule 3 "
                                "(Form 1040), lines 1 through 6z, and the related "
                                "instructions for the types of credits allowed."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 5, "Line 7", "Line 7. Credits."),
                                Source("docs/f1040es_2025.pdf", 8, "Line 7", "7 Credits"),
                            ),
                        ),
                        Field(
                            key="expected_other_taxes",
                            label="Other taxes",
                            summary="Other taxes.",
                            detail=(
                                "Use the 2024 Instructions for Form 1040 to determine whether "
                                "you expect to owe taxes that would have been entered on 2024 "
                                "Schedule 2 (Form 1040), line 8 through 12, 14 through 17z, "
                                "and line 19. Include household employment taxes on this line "
                                "only if you will also have withholding from other income or "
                                "would still need estimated payments without the household "
                                "employment taxes. Do not include taxes that are not due until "
                                "the return due date, such as uncollected Social Security or "
                                "Medicare tax on tips, certain recapture and excise taxes, or "
                                "look-back interest."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 5, "Line 10", "Line 10. Other taxes."),
                                Source("docs/f1040es_2025.pdf", 6, "Additional Medicare Tax / NIIT", "Additional Medicare Tax."),
                            ),
                        ),
                        Field(
                            key="expected_withholding",
                            label="Withholding",
                            summary=(
                                "Income tax withheld and estimated to be withheld during 2025, "
                                "including withholding on pensions, annuities, certain "
                                "deferred income, and Additional Medicare Tax withholding."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 8, "Line 13", "13 Income tax withheld"),
                            ),
                        ),
                        Field(
                            key="prior_year_tax",
                            label="Prior year tax",
                            summary="Required annual payment based on prior year's tax.",
                            detail=(
                                "Enter the 2024 tax figured under the form instructions unless "
                                "an exception applies. If the AGI shown on your 2024 return is "
                                "more than $150,000, or more than $75,000 if married filing "
                                "separately for 2025, use 110% of your 2024 tax instead. If "
                                "you did not file a 2024 return or the 2024 tax year was less "
                                "than 12 full months, do not complete line 12b and use line "
                                "12a on line 12c instead. The instructions also explain how to "
                                "adjust prior-year tax if your joint-filing status changes."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 6, "Line 12b", "Line 12b. Prior year's tax."),
                            ),
                        ),
                    ),
                ),
                Year(
                    year=2026,
                    fields=(
                        Field(
                            key="expected_agi",
                            label="Expected AGI",
                            summary="Adjusted gross income you expect in 2026.",
                            detail=(
                                "When figuring the adjusted gross income you expect in 2026, "
                                "consider the current-year items called out in the form "
                                "instructions. If you are self-employed, subtract the "
                                "deductible part of self-employment tax by using the 2026 "
                                "Self-Employment Tax and Deduction Worksheet."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 7, "Line 1", "Line 1. Adjusted gross income."),
                            ),
                        ),
                        Field(
                            key="expected_deduction",
                            label="Expected deduction",
                            summary="Deductions.",
                            detail=(
                                "If you plan to itemize deductions, enter the estimated total "
                                "of your itemized deductions. If you do not plan to itemize "
                                "deductions, enter your standard deduction plus up to $1,000, "
                                "or $2,000 if married filing jointly, for charitable "
                                "contributions made by cash or check."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 12, "Line 2a", "2a Deductions"),
                            ),
                        ),
                        Field(
                            key="expected_qbi_deduction",
                            label="QBI deduction",
                            summary=(
                                "If you can take the qualified business income deduction, "
                                "enter the estimated amount of the deduction."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 12, "Line 2b", "qualified business income deduction"),
                            ),
                        ),
                        Field(
                            key="expected_amt",
                            label="AMT",
                            summary="Alternative minimum tax from Form 6251.",
                            sources=(
                                Source("docs/f1040es_2026.pdf", 12, "Line 5", "Alternative minimum tax from Form 6251"),
                            ),
                        ),
                        Field(
                            key="expected_credits",
                            label="Credits",
                            summary="Credits. Do not include any income tax withholding on this line.",
                            detail=(
                                "See the 2025 Form 1040 or 1040-SR, line 19, Schedule 3 "
                                "(Form 1040), lines 1 through 6z, and the related "
                                "instructions for the types of credits allowed. The 2026 "
                                "instructions also note that several clean-energy vehicle and "
                                "home-energy credits cannot be claimed in 2026, and that the "
                                "alternative refueling property credit expires for property "
                                "acquired and placed in service after June 30, 2026."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 8, "Line 7", "Line 7. Credits."),
                                Source("docs/f1040es_2026.pdf", 12, "Line 7", "7 Credits"),
                            ),
                        ),
                        Field(
                            key="expected_other_taxes",
                            label="Other taxes",
                            summary="Other taxes.",
                            detail=(
                                "Use the 2025 Instructions for Form 1040 to determine whether "
                                "you expect to owe taxes that would have been entered on 2025 "
                                "Schedule 2 (Form 1040), line 8 through 12, 14 through 17z, "
                                "and line 19. Include household employment taxes on this line "
                                "only if you will also have withholding from other income or "
                                "would still need estimated payments without the household "
                                "employment taxes. Do not include taxes that are not due until "
                                "the return due date, such as uncollected Social Security or "
                                "Medicare tax on tips, certain recapture and excise taxes, or "
                                "look-back interest."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 8, "Line 10", "Line 10. Other taxes."),
                            ),
                        ),
                        Field(
                            key="expected_withholding",
                            label="Withholding",
                            summary=(
                                "Income tax withheld and estimated to be withheld during 2026, "
                                "including withholding on pensions, annuities, certain "
                                "deferred income, and Additional Medicare Tax withholding."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 12, "Line 13", "13 Income tax withheld"),
                            ),
                        ),
                        Field(
                            key="prior_year_tax",
                            label="Prior year tax",
                            summary="Required annual payment based on prior year's tax.",
                            detail=(
                                "Enter the 2025 tax figured under the form instructions unless "
                                "an exception applies. If the AGI shown on your 2025 return is "
                                "more than $150,000, or more than $75,000 if married filing "
                                "separately for 2026, use 110% of your 2025 tax instead. If "
                                "you did not file a 2025 return or the 2025 tax year was less "
                                "than 12 full months, do not complete line 12b and use line "
                                "12a on line 12c instead. The instructions also explain how to "
                                "adjust prior-year tax if your joint-filing status changes."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 8, "Line 12b", "Line 12b. Prior year’s tax."),
                            ),
                        ),
                    ),
                ),
            ),
        ),
        Form(
            id="8995",
            title="Form 8995",
            years=(
                Year(
                    year=2025,
                    fields=(
                        Field(
                            key="expected_qbi_deduction",
                            label="QBI deduction",
                            summary=(
                                "Use Form 8995 to figure the qualified business income "
                                "deduction if your taxable income before the deduction is at "
                                "or below $394,600 for married filing jointly, or $197,300 "
                                "for all other returns, and you are not a patron of a "
                                "specified agricultural or horticultural cooperative."
                            ),
                            detail=(
                                "The deduction can be up to 20% of qualified business income "
                                "from a trade or business, including pass-through income, plus "
                                "20% of qualified REIT dividends and qualified publicly traded "
                                "partnership income, subject to the taxable-income limitation. "
                                "If your taxable income is above the Form 8995 thresholds, use "
                                "Form 8995-A instead."
                            ),
                            sources=(
                                Source("docs/f8995_2025.pdf", 1, "Form header", "Use this form if your taxable income"),
                                Source(
                                    "docs/i8995_2025.pdf",
                                    1,
                                    "Purpose of Form / Who Can Take the Deduction",
                                    "Who Can Take the Deduction",
                                ),
                            ),
                        ),
                        Field(
                            key="line_1",
                            label="Trade, business, or aggregation",
                            summary=(
                                "Enter the name of each trade, business, or aggregation, its "
                                "taxpayer identification number, and its qualified business "
                                "income or loss."
                            ),
                            detail=(
                                "If you aggregated multiple trades or businesses, enter the "
                                "aggregation name and leave the taxpayer identification number "
                                "blank. Enter the employer identification number, or your SSN "
                                "or ITIN if you do not have one."
                            ),
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    4,
                                    "Line 1",
                                    "If you aggregated multiple trades or businesses into a single",
                                ),
                            ),
                        ),
                        Field(
                            key="line_2",
                            label="Total QBI",
                            summary="Add the income and loss from each trade or business in line 1.",
                            detail=(
                                "If you have more than five trades or businesses, attach a "
                                "statement with the name and taxpayer identification number of "
                                "each one, and include their income and loss in this total."
                            ),
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    4,
                                    "Line 2",
                                    "If you have more than five trades or businesses",
                                ),
                            ),
                        ),
                        Field(
                            key="line_3",
                            label="QBI carryforward",
                            summary=(
                                "Enter the qualified part of any trade or business loss "
                                "carryforward allowed in calculating taxable income this year."
                            ),
                            detail=(
                                "Include the carryforward even if the loss came from a trade or "
                                "business that no longer exists. Losses that remain suspended "
                                "by other tax rules are not qualified losses and must be "
                                "tracked separately."
                            ),
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    4,
                                    "Line 3",
                                    "Include here the qualified portion of trade or business (loss)",
                                ),
                            ),
                        ),
                        Field(
                            key="line_4",
                            label="Total qualified business income",
                            summary=(
                                "If you have a qualified business net loss for the year, you do "
                                "not qualify for the QBI deduction unless you have qualified "
                                "REIT dividends or qualified PTP income."
                            ),
                            detail=(
                                "The loss carries forward to next year. The carryforward does "
                                "not change whether the loss is deductible for other purposes."
                            ),
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    4,
                                    "Line 4",
                                    "If you have a qualified business net loss for the year",
                                ),
                            ),
                        ),
                        Field(
                            key="line_6",
                            label="REIT dividends and PTP income",
                            summary=(
                                "Enter qualified REIT dividends and qualified publicly traded "
                                "partnership income or loss."
                            ),
                            detail="Enter income as a positive number and losses as a negative number.",
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    4,
                                    "Line 6",
                                    "Enter income as a positive number and losses as a negative",
                                ),
                            ),
                        ),
                        Field(
                            key="line_7",
                            label="REIT and PTP carryforward",
                            summary=(
                                "Enter the qualified part of any PTP loss carryforward allowed "
                                "in calculating taxable income this year."
                            ),
                            detail=(
                                "Include the carryforward even if you no longer hold an interest "
                                "in the PTP or the PTP no longer exists."
                            ),
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    4,
                                    "Line 7",
                                    "Include here the qualified portion of PTP (loss) carryforward",
                                ),
                            ),
                        ),
                        Field(
                            key="line_8",
                            label="Total REIT and PTP income",
                            summary="Combine lines 6 and 7. If zero or less, enter -0-.",
                            detail="Any negative amount carries forward to next year.",
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    4,
                                    "Line 8",
                                    "Any negative amount will be carried forward to the next year",
                                ),
                            ),
                        ),
                        Field(
                            key="line_11",
                            label="Taxable income before QBI deduction",
                            summary=(
                                "Taxable income before the QBI deduction. This form computes it "
                                "from your expected AGI minus your expected deduction."
                            ),
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    4,
                                    "Line 11",
                                    "Enter your taxable income figured before any QBI",
                                ),
                            ),
                        ),
                        Field(
                            key="line_12",
                            label="Net capital gain and qualified dividends",
                            summary="Enter your net capital gain, increased by any qualified dividends.",
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    4,
                                    "Line 12",
                                    "Enter the amount from your tax return as follows",
                                ),
                            ),
                        ),
                        Field(
                            key="line_15",
                            label="QBI deduction",
                            summary=(
                                "Enter this amount on the QBI deduction line of your return. "
                                "Individual filers enter it on Form 1040 or 1040-SR, line 13a."
                            ),
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    5,
                                    "Line 15",
                                    "Enter this amount on your Form 1040",
                                ),
                            ),
                        ),
                        Field(
                            key="line_16",
                            label="Total QBI carryforward",
                            summary="This amount carries forward to next year and offsets QBI in later years.",
                            detail=(
                                "It offsets QBI in later years whether or not the trade or "
                                "business that generated the loss still exists."
                            ),
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    5,
                                    "Line 16",
                                    "This is the amount to be carried forward to the next year",
                                ),
                            ),
                        ),
                        Field(
                            key="line_17",
                            label="Total REIT and PTP carryforward",
                            summary=(
                                "This amount carries forward to next year and offsets qualified "
                                "REIT dividends and PTP income in later years."
                            ),
                            sources=(
                                Source(
                                    "docs/i8995_2025.pdf",
                                    5,
                                    "Line 17",
                                    "This amount must be carried forward to next year",
                                ),
                            ),
                        ),
                    ),
                ),
            ),
        ),
        Form(
            id="se-worksheet",
            title="Self-Employment Tax and Deduction Worksheet",
            years=(
                Year(
                    year=2025,
                    fields=(
                        Field(
                            key="se_income",
                            label="Expected SE income",
                            summary="Enter your expected income and profits subject to self-employment tax.",
                            detail=(
                                "Your net profit from self-employment is found on Schedule C "
                                "(Form 1040), line 31; Schedule F (Form 1040), line 34; and "
                                "Schedule K-1 (Form 1065), box 14, code A."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 6, "Line 1a", "1a. Enter your expected income and profits"),
                            ),
                        ),
                        Field(
                            key="crp_payments",
                            label="Expected CRP payments",
                            summary=(
                                "If you will have farm income and also receive Social Security "
                                "retirement or disability benefits, enter your expected "
                                "Conservation Reserve Program payments that will be included on "
                                "Schedule F (Form 1040) or listed on Schedule K-1 (Form 1065)."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 6, "Line 1b", "expected Conservation Reserve"),
                            ),
                        ),
                        Field("line_2", "Line 2", "Subtract line 1b from line 1a.", sources=(Source("docs/f1040es_2025.pdf", 6, "Line 2", "2. Subtract line 1b from line 1a"),)),
                        Field("line_3", "Line 3", "Multiply line 2 by 92.35% (0.9235).", sources=(Source("docs/f1040es_2025.pdf", 6, "Line 3", "3. Multiply line 2 by 92.35%"),)),
                        Field("line_4", "Line 4", "Multiply line 3 by 2.9% (0.029).", sources=(Source("docs/f1040es_2025.pdf", 6, "Line 4", "4. Multiply line 3 by 2.9%"),)),
                        Field("line_5", "Line 5", "Social Security tax maximum income: $176,100.", sources=(Source("docs/f1040es_2025.pdf", 6, "Line 5", "5. Social security tax maximum income"),)),
                        Field(
                            key="expected_wages",
                            label="Expected wages",
                            summary=(
                                "Enter your expected wages if they are subject to Social "
                                "Security tax or the 6.2% portion of tier 1 railroad "
                                "retirement tax."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 6, "Line 6", "6. Enter your expected wages"),
                            ),
                        ),
                        Field(
                            key="line_7",
                            label="Line 7",
                            summary="Subtract line 6 from line 5.",
                            detail="If line 7 is zero or less, enter 0 on line 9 and skip to line 10.",
                            sources=(
                                Source("docs/f1040es_2025.pdf", 6, "Line 7 / Note", "Note. If line 7 is zero or less"),
                            ),
                        ),
                        Field("line_8", "Line 8", "Enter the smaller of line 3 or line 7.", sources=(Source("docs/f1040es_2025.pdf", 6, "Line 8", "8. Enter the smaller of line 3 or line 7"),)),
                        Field("line_9", "Line 9", "Multiply line 8 by 12.4% (0.124).", sources=(Source("docs/f1040es_2025.pdf", 6, "Line 9", "9. Multiply line 8 by 12.4%"),)),
                        Field(
                            key="line_10",
                            label="Line 10",
                            summary=(
                                "Add lines 4 and 9. Enter the result here and on line 9 of "
                                "your 2025 Estimated Tax Worksheet."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 6, "Line 10", "10. Add lines 4 and 9."),
                            ),
                        ),
                        Field(
                            key="line_11",
                            label="Line 11",
                            summary="Multiply line 10 by 50% (0.50).",
                            detail=(
                                "This is your expected deduction for self-employment tax on "
                                "Schedule 1 (Form 1040), line 15. Subtract this amount when "
                                "figuring your expected AGI on line 1 of your 2025 Estimated "
                                "Tax Worksheet."
                            ),
                            sources=(
                                Source("docs/f1040es_2025.pdf", 6, "Line 11", "11. Multiply line 10 by 50%"),
                            ),
                        ),
                    ),
                ),
                Year(
                    year=2026,
                    fields=(
                        Field(
                            key="se_income",
                            label="Expected SE income",
                            summary="Enter your expected income and profits subject to self-employment tax.",
                            detail=(
                                "Your net profit from self-employment is found on Schedule C "
                                "(Form 1040), line 31; on Schedule F (Form 1040), line 34; "
                                "and in box 14, code A, of Schedule K-1 (Form 1065)."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 9, "Line 1a", "1a. Enter your expected income and profits"),
                            ),
                        ),
                        Field(
                            key="crp_payments",
                            label="Expected CRP payments",
                            summary=(
                                "If you will have farm income and also receive Social Security "
                                "retirement or disability benefits, enter your expected "
                                "Conservation Reserve Program payments that will be included on "
                                "Schedule F (Form 1040) or listed on Schedule K-1 (Form 1065)."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 9, "Line 1b", "expected Conservation Reserve"),
                            ),
                        ),
                        Field("line_2", "Line 2", "Subtract line 1b from line 1a.", sources=(Source("docs/f1040es_2026.pdf", 9, "Line 2", "2. Subtract line 1b from line 1a"),)),
                        Field("line_3", "Line 3", "Multiply line 2 by 92.35% (0.9235).", sources=(Source("docs/f1040es_2026.pdf", 9, "Line 3", "3. Multiply line 2 by 92.35%"),)),
                        Field("line_4", "Line 4", "Multiply line 3 by 2.9% (0.029).", sources=(Source("docs/f1040es_2026.pdf", 9, "Line 4", "4. Multiply line 3 by 2.9%"),)),
                        Field("line_5", "Line 5", "Social Security tax maximum income: $184,500.", sources=(Source("docs/f1040es_2026.pdf", 9, "Line 5", "5. Social security tax maximum income"),)),
                        Field(
                            key="expected_wages",
                            label="Expected wages",
                            summary=(
                                "Enter your expected wages if they are subject to Social "
                                "Security tax or the 6.2% portion of tier 1 railroad "
                                "retirement tax."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 9, "Line 6", "6. Enter your expected wages"),
                            ),
                        ),
                        Field(
                            key="line_7",
                            label="Line 7",
                            summary="Subtract line 6 from line 5.",
                            detail="If line 7 is zero or less, enter 0 on line 9 and skip to line 10.",
                            sources=(
                                Source("docs/f1040es_2026.pdf", 9, "Line 7 / Note", "Note: If line 7 is zero or less"),
                            ),
                        ),
                        Field("line_8", "Line 8", "Enter the smaller of line 3 or line 7.", sources=(Source("docs/f1040es_2026.pdf", 9, "Line 8", "8. Enter the smaller of line 3 or line 7"),)),
                        Field("line_9", "Line 9", "Multiply line 8 by 12.4% (0.124).", sources=(Source("docs/f1040es_2026.pdf", 9, "Line 9", "9. Multiply line 8 by 12.4%"),)),
                        Field(
                            key="line_10",
                            label="Line 10",
                            summary=(
                                "Add lines 4 and 9. Enter the result here and on line 9 of "
                                "your 2026 Estimated Tax Worksheet."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 9, "Line 10", "10. Add lines 4 and 9."),
                            ),
                        ),
                        Field(
                            key="line_11",
                            label="Line 11",
                            summary="Multiply line 10 by 50% (0.50).",
                            detail=(
                                "This is your expected deduction for self-employment tax on "
                                "Schedule 1 (Form 1040), line 15. Subtract this amount when "
                                "figuring your expected AGI on line 1 of your 2026 Estimated "
                                "Tax Worksheet."
                            ),
                            sources=(
                                Source("docs/f1040es_2026.pdf", 9, "Line 11", "11. Multiply line 10 by 50%"),
                            ),
                        ),
                    ),
                ),
            ),
        ),
    )


def quote(text: str) -> str:
    escaped = text.replace("\\", "\\\\").replace('"', '\\"')
    return '"' + escaped + '"'


def quote_multiline(text: str) -> str:
    escaped = text.replace('"""', '\\"\\"\\"')
    return '"""' + escaped + '"""'


def write_lines(lines: Iterable[str]) -> str:
    return "\n".join(lines) + "\n"


def render_toml(all_forms: Iterable[Form]) -> str:
    lines: list[str] = []
    for form in all_forms:
        lines.extend(("[[forms]]", f"id = {quote(form.id)}", f"title = {quote(form.title)}", ""))
        for year in form.years:
            lines.extend(("  [[forms.years]]", f"  year = {year.year}", ""))
            for field_ in year.fields:
                lines.extend(
                    (
                        "    [[forms.years.fields]]",
                        f"    key = {quote(field_.key)}",
                        f"    label = {quote(field_.label)}",
                        f"    summary = {quote(field_.summary)}",
                    )
                )
                if field_.detail is not None:
                    lines.append(f"    detail = {quote_multiline(field_.detail)}")
                lines.append("")
                for source in field_.sources:
                    lines.extend(
                        (
                            "      [[forms.years.fields.sources]]",
                            f"      file = {quote(source.file)}",
                            f"      page = {source.page}",
                            f"      section = {quote(source.section)}",
                            "",
                        )
                    )
    return write_lines(lines)


def validate_anchors(all_forms: Iterable[Form]) -> None:
    pdf_cache: dict[tuple[str, int], str] = {}
    for form in all_forms:
        for year in form.years:
            for field_ in year.fields:
                for source in field_.sources:
                    cache_key = (source.file, source.page)
                    page_text = pdf_cache.get(cache_key)
                    if page_text is None:
                        reader = PdfReader(REPO_ROOT / source.file)
                        page_text = (reader.pages[source.page - 1].extract_text() or "").replace("\x00", " ")
                        pdf_cache[cache_key] = page_text
                    if source.anchor not in page_text:
                        raise SystemExit(
                            f"Missing anchor {source.anchor!r} in {source.file} page {source.page}"
                        )


def main() -> None:
    all_forms = forms()
    validate_anchors(all_forms)
    OUTPUT_PATH.write_text(render_toml(all_forms), encoding="utf-8")
    print(f"Wrote {OUTPUT_PATH.relative_to(REPO_ROOT)}")


if __name__ == "__main__":
    main()

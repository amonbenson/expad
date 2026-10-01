# Writing Guide for Course Reports

Content and language rules for a project report at TU Berlin, Chair of Sensor and Actuator Systems.
Derived from the accepted report *Simulation of a Percussive Synthesizer with Macro Controls*
(drumsynth-report). The rules are independent of the writing tool. Generic rules (a report has a title
page, a bibliography, page numbers) are omitted.

## 0. Index

| Looking for | Section |
|---|---|
| Meaning of the words used in this guide | [1. Terms](#1-terms) |
| Which parts the report has, in which order | [2. Document Order](#2-document-order) |
| What each chapter contains | [3. Chapters](#3-chapters) |
| How a section and a paragraph are built | [4. Sections and Paragraphs](#4-sections-and-paragraphs) |
| Person, voice, tense | [5. Voice and Tense](#5-voice-and-tense) |
| Sentence construction, punctuation | [6. Sentences](#6-sentences) |
| Technical terms, abbreviations, names of parts and signals | [7. Naming](#7-naming) |
| Figures, tables, equations | [8. Figures, Tables and Equations](#8-figures-tables-and-equations) |
| Citations | [9. Citations](#9-citations) |
| Numbers and units | [10. Numbers and Units](#10-numbers-and-units) |
| Tone | [11. Tone](#11-tone) |
| Final check | [12. Checklist](#12-checklist) |

## 1. Terms

| Term | Meaning in this guide |
|---|---|
| report | The whole document handed in. |
| project | The practical work the report documents. |
| chapter | A numbered top-level part (Introduction, Implementation, ...). |
| section | A numbered part inside a chapter. |
| float | A figure or a table. |
| caption | The text attached to a float: a short title and a description. |
| source | An external publication, datasheet, product page or repository listed in the bibliography. |
| citation | The numbered reference to a source in the text, e.g. [4]. |
| block | One functional unit of the project (a circuit, a module, a program part). |
| design value | A chosen quantity of the project (a resistance, a rate, a threshold). |

## 2. Document Order

1. Title page: university, faculty, institute, chair; document type (`Report`); title; `In the course` with
   course name and submission date; author with matriculation number; supervisor (professor) and
   `Assisted by` (assistant).
2. Statutory declaration (`Eidesstattliche Erklärung`): the institution's fixed German text, place, date,
   signature line. The fixed text requires that every AI tool is named in the report.
3. `Notice on the Use of AI Tools`: unnumbered chapter, one paragraph (see [3.1](#31-notice-on-the-use-of-ai-tools)).
4. `Abstract`: unnumbered chapter.
5. Table of contents.
6. Numbered chapters: Introduction, Related Work, (Theory), Implementation, (Results), Discussion,
   Conclusion and Outlook. Chapters in parentheses are optional; their content may live in Implementation.
7. Appendix: one chapter, sections per item (listings, tables of design values).
8. Bibliography, list of figures, list of tables.

## 3. Chapters

### 3.1 Notice on the Use of AI Tools

- One paragraph. One sentence per tool: product name, manufacturer, version, purpose, citation of the tool.
- Name the concrete work product the tool contributed to (e.g. "the Python script for extracting ...")
  and cite the repository holding it.
- Final sentence: all AI-generated content was reviewed and verified by the author, who takes full
  responsibility for the final content.

### 3.2 Abstract

- One paragraph, about 10 sentences, no citations, no abbreviations without their full form.
- Sentence order: context of the field -> problem -> what the project presents -> its parts (one sentence
  listing them with a dash-enclosed enumeration) -> how the central part is realized -> how the design was
  developed and verified -> results -> one concluding sentence on what the results show.
- The abstract states results; it does not announce that results will be shown.

### 3.3 Introduction

- Purpose: background, the niche the project occupies, motivation, the minimal fundamentals needed to
  follow the report.
- Paragraph order: (1) the field and why it matters, with sources; (2) the underlying structure or
  principle of the problem domain, with sources; (3) the concrete difficulty, with sources showing it is
  known; (4) "Concretely, ..." the project's approach in two to four sentences; (5) the development and
  verification tool or method, with a source showing it has worked before.
- No figures. No results. No chapter-by-chapter roadmap.

### 3.4 Related Work

- One section per group of existing work. Name the group in italics when it is defined
  (e.g. *Per-voice analog clones*), then give two or more examples with manufacturer and citation.
- Each section ends with one sentence that places the project against the group: what none of them
  combine, or how the project applies the same principle to a different target.
- Short: two to five sentences per section. No figures.

### 3.5 Implementation

- Opens with an overview paragraph and a block diagram: which blocks exist and how signals flow between
  them. One sentence per block.
- Then one sentence on how blocks were developed and tested individually.
- Then one section per block, in signal-flow order (see [4](#4-sections-and-paragraphs)).
- A development observation that changed the method (e.g. simulation too slow, so intermediate signals are
  cached) is reported where it occurred, with its consequence.
- Last section: `Results`. It describes the assembled system, the test that was run (conditions in the
  caption), and what the output shows. Results are statements about the figure, not interpretation.

### 3.6 Discussion

- No new results, no figures.
- Paragraph order: (1) what was shown, in one or two sentences, and its limit (e.g. linear only);
  (2) evaluation of the method or tool: what worked, what failed, how the failure was mitigated;
  (3) limitations of the current design, each with the known remedy and a source.
- Prose only, compact paragraphs.

### 3.7 Conclusion and Outlook

- Paragraph order: (1) one manual or limited step of the project and how it could be automated or
  extended, with sources for the methods; (2) a second extension; (3) one closing sentence beginning
  "Overall, this work demonstrates that ...".
- No new facts about the project.

### 3.8 Appendix

- Material referenced from the text but too long for it: listings, full tables of design values, scripts.
- Each item is a section with a label so the text can reference it.

## 4. Sections and Paragraphs

- A section about a block follows this order:
  1. Purpose in context: what gap the previous blocks leave and how this block fills it.
  2. Physical or algorithmic principle, with a source.
  3. Realization, walking through the figure: component by component, in the order the signal passes.
  4. Derivation of the governing equations, step by step.
  5. Design values: computed from the equations, given in a table if there are more than three.
  6. Floats.
- The first sentence of a section links to what the reader already knows
  (e.g. "While the two noise generators ..., the pitched oscillator ...";
  "The three presented signal sources generate continuous output. For an accurate ..., however, ...").
- One paragraph has one topic. A paragraph has three to eight sentences.
- Every design choice is followed in the same or the next sentence by its reason:
  "X is therefore preferred over Y, whose ... is not suited to ...".
- An intentional choice that looks unusual is marked as intentional and justified
  ("This comparatively high cutoff is chosen intentionally, as ...").
- A simulation or measurement artefact is named as such and separated from the property of the circuit
  ("... is a simulation artifact of ... rather than a property of the circuit").
- Body text contains no bullet lists. Enumerations are written as sentences.

## 5. Voice and Tense

- No first person (no "I", "we", "our"). The project or the report is the subject ("This project
  presents ...", "this work demonstrates ..."), or the sentence is passive.
- "The author" appears only in the AI notice.
- Present tense for the design and how it works ("The capacitor is charged ...", "The matrix is realized
  as ...").
- Present passive for the method as a procedure ("A dedicated test circuit is then used ...").
- Past tense for events of the project and for findings ("It was shown that ...", "LTspice proved ...",
  "During development, it became clear that ...", "the weights were determined by hand").
- Results chapter and captions describe the figure in present tense ("The first hit blends ...").

## 6. Sentences

- One sentence states one concept.
- Typical length 15 to 30 words. Split a sentence that needs two "and"-joined main clauses.
- A colon introduces the elaboration of a general statement
  ("... share a consistent base structure: a short broadband attack transient, followed by ...").
- A dash pair (en dash with spaces) encloses an inline enumeration or an aside
  ("three sound sources – an analog ..., a ..., and a ... – each shaped by ...").
- Connectives that carry logic, placed at the start or after the subject: "therefore", "however",
  "instead", "thus", "additionally", "concretely".
- Causal chains are explicit: "As X falls, Y decreases, producing Z."
- Quantitative statements carry a number: "around 10 V peak-to-peak", not "a large amplitude".
- Informal words in quotation marks only when meant figuratively ("listen" to a waveform).

## 7. Naming

- One term per concept for the whole report. Define it at first use; never switch to a synonym.
- Abbreviation: full form at first use, abbreviation in parentheses ("digital musical instrument (DMI)").
  Write out the full form again in the abstract.
- Category names are italic at the point of definition only.
- Components are named with their schematic designator in math italics ($R_5$, $C_2$, $Q_3$, $U_1$),
  identical to the figure.
- Signals and nodes are named exactly as in the tool output (e.g. `V(gate)` in monospace); the caption
  defines each one shown.
- Part numbers in upright text (TL072, OPA1678) with a datasheet citation at first mention.
- Commercial products: manufacturer and product name ("Mutable Instruments Plaits"), cited.
- Software and tools: product name, cited at first mention.
- Code identifiers in monospace, only when the reader needs them to find the code.

## 8. Figures, Tables and Equations

- Every float is referenced in the text before or at the position it appears, by its full name
  ("Figure 3.2", "Table 3.1"). The reference is part of the sentence ("shown in Figure 3.1") or a
  parenthesis after the claim ("(Figure 3.4)").
- Caption form: short title for the list of figures, then the caption itself:
  **Title in Title Case** – one to four sentences.
  The sentences say what the float shows, define every label, symbol and signal in it, and state the
  test conditions (input, duration, values). A caption is understandable without the body text.
- Sub-figures carry a Title Case sub-caption and are referenced as (a), (b), ... in the main caption.
- A schematic and its waveform are grouped as sub-figures of one figure.
- Tables: bold header row, units in the header or with each value, caption with the source of the values.
- Images from tools (schematics, plots) are used as produced; the caption explains what to look at.
- Equations: numbered when referenced or derived. Derivations show every step that changes the form; a
  condition is written as "set equal" (an exclamation mark over the equals sign) followed by the solved
  form. Subscripts that are words are upright (t_dis, V_DD).
- Every symbol is defined in the sentence before or directly after its first equation.
- A derived formula is immediately applied to a design value with units
  ("this yields a current of I_E = ... = 6 µA, which is realistic for that transistor model [12]").
- Vectors and matrices bold.

## 9. Citations

- Numeric style (IEEE), sorted by name, year, title.
- Every external fact carries a citation: physical effects, prior circuits, part properties, product
  features, historical claims.
- The citation stands at the end of the claim it supports, before the full stop; several sources share one
  bracket.
- An author-name citation ("As Werner et al. explain ...") when the text builds on one source's
  explanation.
- Datasheets, product pages and blog posts are valid sources; give manufacturer as author, document
  number, revision, URL and access date.
- The project's own repository is a source and is cited where its code or data is used.
- Physical-principle sources are textbooks or papers; component-behaviour sources are datasheets.

## 10. Numbers and Units

- SI units with a (thin) space between number and unit: 10 kΩ, 4.8 kHz.
- Ranges with the unit on both ends or a range form: -12 V to 12 V.
- Intermediate calculations shown with units, the result rounded to the precision that matters
  (≈ 4.8 kHz).
- Dimensionless values to three significant digits in tables.
- Equal precision within one table column.

## 11. Tone

- Documentation, not advertisement. No "novel", "innovative", "state-of-the-art", "powerful".
- Significance is stated once (Introduction, Related Work closing sentences, Discussion first paragraph),
  not in every chapter.
- Evaluative words ("valuable", "well suited") only with the reason in the same sentence.
- Limitations are stated as facts with their cause and remedy, not apologised for.
- No rhetorical questions, no exclamations, no addressing the reader.

## 12. Checklist

- [ ] Document order as in section 2; AI notice names product, manufacturer, version, purpose.
- [ ] Abstract one paragraph, ~10 sentences, no citations.
- [ ] No first person; tenses as in section 5.
- [ ] No bullet lists in body text.
- [ ] Each section opens with a link to known content and follows the order in section 4.
- [ ] Each design choice has its reason; each external fact has its citation.
- [ ] Each float referenced in text; each caption has title, description, label definitions, conditions.
- [ ] Each symbol defined; each formula applied to a value with units.
- [ ] One term per concept; abbreviations defined at first use.
- [ ] Discussion and Conclusion contain no new results.

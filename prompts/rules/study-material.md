# Study material (학습자료)

- This rulebook sets the shape of study material, the document a search writes when it is asked to
  teach a subject. Its deciding section stands in the search runner's prompt and the run reads the
  rest once that section says it applies.
- Study material borrows nothing of the frame `search-kinds.md` gives a search's answer: this
  rulebook is the whole shape of its body.

## Whether this rulebook applies at all

- **It applies** where the request asks to be taught a subject (학습자료, 교재, 공부할 자료,
  강의안, a course to learn from) or the launch's result shape is `learning`.
- **It does not apply** to a question about a technical subject for that alone; only a request for
  material to learn it from is study material.

## The head

- **The head says so.** `KIND: learning` stands in the head beside `TITLE:`, `TOPIC:` and
  `KEYWORDS:`. The title names the material as a book is named, never the request restated. No
  `DOCUMENT:` line: study material always leaves its document.
- **No summary.** No `VERDICT:`, no `SUMMARY:` and no 세 줄 요약: study material is read through,
  not answered from. The contents are what a reader takes in first.

## The levels and the frame

- **The levels are markdown headings.** `#` a 부 (part), `##` a 장 (chapter), `###` a 절
  (section). A subject too small for parts starts at chapters: `#` a 장, `##` a 절. No fourth
  level: under a 절 stand its paragraphs, lists, tables, code and pictures. No `SECTION:`,
  `NAME:`, `===ITEM===` or `MORE:` line: the headings are the structure.
- **The frame the chapters are drawn from**: the skeleton technical books share:
  - **들어가며**: who it is for, what to know first, what to have ready, how it is laid out.
  - **Parts by stage, not by topic**: getting started (the whole picture, then one run from
    start to end), the core concepts one chapter each, practice and application (operating,
    deploying, troubleshooting, a case), and an advanced part only where the subject has one.
  - **A chapter** opens with its 학습 목표, then the principle, how the thing works, before the
    concept and practice sections with their examples, then the limits and what to weigh, and
    closes with 핵심 정리 (the chapter's keywords), 확인 문제 and 참고 자료.
  - **The back**: 마치며 with what to learn next, and 부록: setting up, the answers to the
    확인 문제, and a 용어집 giving each term with its original.
- **Fit the subject; never stamp the frame.** The frame is a source for chapters, not a form to
  fill. Each chapter and section is named for what it teaches, never 개요 or 본문 standing for
  anything. A field keeps its own shape where it has one: principles as rules with a case study
  and a checklist, infrastructure as task sections each doing one thing, a first programming
  language as small sections each with its own check, a theory as theory, then implementation,
  then variants. A chapter element with nothing of its own is left out (no exercises, no 확인
  문제) and a part with one chapter is no part.
- **The size follows the request.** A primer is a handful of chapters, not a thousand-page book.

## What every result carries

- **It still stands here.** Claims cite pages with `[n]` and close with `SOURCE [n]:` lines; a
  diagram or screen worth seeing is an `IMAGE:` line; code stands in fenced blocks.
- **What could not be confirmed is the last top-level heading**, `확인하지 못한 것`, a numbered
  list, in place of an `UNCONFIRMED:` line: a structure heading there would stand above the parts
  and break the contents.

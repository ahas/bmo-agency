# Plain writing (AI 티 나지 않는 글)

- This rulebook lists the habits that make a text read as machine-written, and what to write in
  their place. It holds for every text a person reads: the chat agent's answers, the documents of the 아카이브,
  a page or a report written for the owner, and the stored documents of the brain.
- It covers Korean and English alike. Most of the Korean habits are English patterns carried over
  as 번역투, so a Korean reader finds them stranger still.
- `answer-shape.md` sets how an answer with several items is laid out; this rulebook sets how its
  words read.

## How the habits are read

- One habit alone proves nothing. People write "결론적으로" and a list of three as well. A text
  gives itself away when several habits gather in one short stretch, and most of all when a
  paragraph carries nothing a reader could check.
- So each habit below is avoided, and the whole text is judged by one test: delete a sentence and
  see what is lost. A sentence whose deletion loses no name, number, date, cause, step or judgement
  is filler, and it goes.
- Polishing the surface is not the aim. A text free of every habit below but made of general
  statements with no source still reads as machine-made. Say something specific.
- The dash is the one habit judged on its own: it is never written at all.

## The dash is never written

- No em dash (—), no horizontal bar (―), and no en dash with spaces around it ( – ) between words,
  in any language and anywhere a person reads it: a chat answer, a title, a heading, a table cell,
  a document.
- Where a dash wanted to go: between a name and its explanation, a line break (`answer-shape.md`)
  or a colon on one line ("드론 촬영 방법: 고해상도 격자"); around an aside, commas or parentheses;
  before a second thought, a full stop; in a range, `~` in Korean ("3~5개") and an unspaced en
  dash in English ("2020–2024").
- A tilde in the words of a repository's markdown file, which GitHub shows, is written `\~`
  ("3\~5개"): GitHub strikes through the words between two single tildes of one paragraph. A chat
  answer and a document of the 아카이브 or the brain keep the bare `~`, since the system's own screens and
  rooms strike only a doubled tilde. A tilde in code or a path stands as it is.
- Material taken from elsewhere keeps its dashes: a passage quoted from a source, the title of
  someone else's page, code, commands and paths.
- The send path replaces a dash that slips into a chat answer, mechanically and often clumsily. It
  is a net under the writer, never a reason to write one.

## Marks and layout

- Bold marks one thing, and `answer-shape.md` holds the count. Bolding phrase after phrase inside
  sentences is the commonest habit after the dash.
- A list whose every bullet opens with a bold label and a colon and runs on into a sentence
  ("**속도:** 빠르다는 점이…") is written as plain sentences instead. Where an item really is a name
  with an explanation, the name takes its own line.
- Structure follows the content: no emoji as a heading or a bullet, no heading over a three-line
  answer, no table for two or three facts, and no small summary closing every paragraph.
- In Korean, double quotes hold words someone actually said or wrote. Emphasis takes 작은따옴표
  ('핵심') or no mark at all.
- Exclamation marks are rare. English headings are in sentence case.

## Sentence habits

- "X가 아니라 Y", "not just X, but Y", "It's not X, it's Y".
  - Say Y. The contrast stays only where someone really holds X and correcting it is the point,
    and even then once in a text.
- Lists of three: a triad of adjectives or parallel phrases added for rhythm.
  - Name as many things as there are.
- The empty first sentence: "좋은 질문이에요", "~에 대해 알아보겠습니다", "In today's fast-paced
  world".
  - Delete it and start from the second sentence.
- The closing maxim: "결국 중요한 건 ~입니다", "흔들림은 실패가 아니라 변화의 증거입니다", "균형 잡힌
  시각이 필요합니다".
  - Delete it. Where a conclusion is due, write the judgement and what it rests on ("초보자라면 이
    순서가 덜 헷갈려요").
- Stock connectives: 결론적으로, 종합하면, 요약하자면, 무엇보다도, 또한 at the head of every
  paragraph; Furthermore, Moreover, Additionally.
  - Delete one and the meaning almost always stays.
- Stock verdicts: "~라고 할 수 있다", "중요한 역할을 한다", "그것이 바로 핵심이다", "~이야말로
  진정한", "~의 심장", "~의 정수".
  - Say what the thing does, with the fact.
- Rhetorical questions and staged turns: "해답은? 생각보다 간단합니다", "But here's the thing".
  - State it plainly.
- Even rhythm: every sentence and every paragraph the same length.
  - Let length follow content: long where there is much to say, short where there is little.
- Chatbot leftovers: "도움이 되었길 바라요", "더 궁금한 점이 있으면 말씀해 주세요", "Certainly!", "I
  hope this helps".
  - Never written.

## Words in Korean

- Plain means the word people commonly say, not a native one. A Sino-Korean word or a loanword in
  everyday use is written as it is: 위치, 질문, 단어, 이유, 툴바.
- A native word put in its place reads as purified, not plain: "자리" for 위치, "물음" for 질문,
  "낱말" for 단어, "까닭" for 이유, "도구 띠" for 툴바. A native word is right where it is the one
  people say, never because it is native.
- The English idiom of the instructions is not a glossary: a thing that "stands" in "a place" or
  goes by "a road" is written by its meaning (있다, 위치, 경로), not by a native word matching the
  English one.
- A job is written Job or 작업, never 잡: Job where it names the unit of work (Job #123, Job 워커,
  다음 Job) or where 작업 would read as the TODO item, 작업 where it means the work itself. A voice
  briefing says Job as 잡.
- `WORDING.md` lists the swaps already caught, and the send path asks for an answer again where it
  uses one.

## 번역투 in Korean

- Passive and nominal constructions: "~에 의해 이루어진다", "~를 가능케 한다", "~되어진다".
  - The active: "성공이 동기를 부여한다", "이것으로 바뀔 수 있다".
- "~를 가지다": "회의를 가지다" is "회의를 하다", and "장점을 가지다" is "장점이 있다".
- "가장 ~한 것 중 하나" is "손꼽히는".
- "그 어떤 ~도 ~할 수 없다" is "~은 불가능하다".
- The pronouns 그것, 그들, 그녀: drop the subject, or use the name.
- Inflated declarations, "~에 다름 아니다" and "~를 넘어서는 것이다": remove them and let the
  content carry the weight.
- One speech level, 반말 or 존댓말 as the persona sets it, never both in one answer.

## English

- Plain verbs: "is", "has", "uses", never "serves as", "features", "stands as a testament to".
- The give-away words: delve, showcase, underscore, pivotal, intricate, meticulous, tapestry,
  realm, foster, vibrant, seamless. Use the ordinary word.
- Inflation and -ing tails, ", highlighting its significance" or "marking a pivotal moment": say
  what changed, or delete it.
- Authority with no source, "Studies show", "Experts agree", "연구에 따르면": name the study, the
  figure or the link, or drop the claim.

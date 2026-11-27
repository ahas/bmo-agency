# The shell's verbs

`bmo-agency` is agency's command on the operations machine, reached as `bmo <noun> …` through the set's
dispatcher. Its nouns are listed by `bmo-agency nouns`, each marked `home` where its commands act on the operations
machine.

## One grammar for every noun

- **A collection of agency's rows is worked through five steps of one shape,** so what an agent learns on one noun
  holds on every other:

  | Step | Does | Route |
  |---|---|---|
  | `list` | every row, one a line: its name, and its role, provider, model, effort or path where it has one, `off` where it is turned off | `GET` of the collection |
  | `show <name>` | one row whole, as the route answers it | `GET` of the row |
  | `add <name> --set field=value …` | a row added under the name with the fields named | `POST` of the collection |
  | `set <name> --set field=value …` | the fields named changed, every other field left as it stands | `PATCH` of the row |
  | `remove <name>` | the row removed | `DELETE` of the row |

- **The nouns that take it:** `role-agent` (a role's seats, `/role-agents`) takes all five; `repository` takes all
  five, its `add` keeping its own `--path`, `--label` and `--umbrella`; `supervisor` takes `list`, `show` and `set`,
  a supervisor being added and removed through the receiver, which keeps its room; `model` keeps its own steps of
  the same names (`docs/model-catalogue.md`).
- **A change names only what it changes.** Each `--set field=value` is one field of the route's body, its value
  read as JSON where it reads as JSON — `enabled=false` the boolean, `answerTokens=8000` the number — and as text
  otherwise, `model=q38fn-dq8`. A `set` naming nothing is refused before anything is asked.
- **Every step goes through the server's own routes** with the deployment's control token, so the route's own
  checks and refusals stand as they stand for the console. A name holds letters, digits, dots, dashes and
  underscores, as every row's name does; another is refused before anything is asked.
- **A refusal says the route's words and the step past it:** a name the collection does not hold points to `list`, a
  name taken to `set`, a field out of shape to `show`, which shows a row's fields as they stand.

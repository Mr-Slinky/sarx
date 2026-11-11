---
status: accepted
date: 2026-10-17
decision-makers: Kheagen Haskins
---

# Central AI

## Context and Problem Statement

Villages advance through new technologies and stages without the player directing each step. Every
villager in a village needs work, such as chopping wood, farming or building. The AI decides who
does what.

The simulation advances in fixed steps, at the same rate on every device. Sarx also runs in full on
a mid-level laptop. The work of the AI in each step therefore has to finish within the time between
steps on that laptop, however many villagers the world contains.

The creature has an AI of its own, which the player trains through reward and punishment. That AI is
a separate planned feature. This record covers the villagers alone.

How does the AI decide what each villager does?

## Decision Drivers

- The time that the AI takes in each step stays within the step as villages grow.
- A village acts as a group. It might, for example, send a set number of villagers to one job.

## Considered Options

- Central planners, one per group, with each villager carrying out a simple task
- An agent per villager, with each villager deciding for itself

## Decision Outcome

Chosen option: "Central planners, one per group, with each villager carrying out a simple task",
because the planning work then grows with the number of decisions rather than with the number of
villagers.

A planner is the part of the AI that decides for one group, such as a village. The planner of a
village might send six villagers to chop wood. Each of those villagers then carries out a simple
task.

Three points remain open. The developer settles them once the planner is built:

- the technique that a planner uses to decide, such as goal-oriented action planning (GOAP) or
  utility AI
- how often a planner decides
- which groups other than a village have a planner of their own

### Consequences

- Good, because the planning work grows with the number of decisions. Take a village of 200
  villagers that sends six of them to chop wood. Its planner makes one decision about wood. With an
  agent per villager, all 200 villagers decide what to do.
- Good, because the planner has the whole village in view. It can therefore match the number of
  villagers on each job to what the village needs.
- Good, because the code that runs for each villager in each step carries out one task. As a result,
  that code stays small.
- Bad, because villagers given the same task behave alike. Any variety between them, such as one
  villager working more slowly than another, has to be built into the task itself.
- Bad, because a villager responds to a change around it, such as an approaching enemy, only as far
  as its task allows. The villager responds in full only once its planner decides again.

### Confirmation

The AI has yet to be written. The TODO in the README lists the central AI planner under the
simulation work, after the foundation.

Once that code exists, a reader confirms this decision with two checks:

- Each group has a planner that assigns tasks.
- The code that runs for each villager carries out the task that its planner assigned.

## Pros and Cons of the Options

### Central Planners, One per Group, with Each Villager Carrying Out a Simple Task

A planner decides for its whole group, and assigns each villager a task.

- Good, because the planning work grows with the number of decisions rather than with the number of
  villagers.
- Good, because one planner coordinates the labour of a group, with the whole group in view.
- Good, because the code for each villager stays small. It carries out one task at a time.
- Bad, because villagers given the same task act alike.
- Bad, because a villager reacts to a change around it within the limits of its task, until its
  planner decides again.

### An Agent per Villager, with Each Villager Deciding for Itself

Each villager runs its own decision-making. At each decision, it weighs its needs and its
surroundings. It then picks its next action.

- Good, because each villager reacts at once to what happens around it.
- Good, because each villager weighs its own situation. Villagers therefore behave differently from
  one another.
- Bad, because the work of deciding grows with the number of villagers. Every villager decides for
  itself.
- Bad, because villagers who decide alone coordinate only through shared state. A record of which
  villager has taken which job is one example. Without such a record, every villager can choose the
  same tree. All of them can also choose wood while the village runs short of food.

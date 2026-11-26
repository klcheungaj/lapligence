# Members of static class properties

A static class property is one variable of its class; a path that names it,
through the class, an object, `this`, `super` or a virtual interface, may go on
into a member, element, bit or part of it. The path used to give the property
as a whole: `st::pr.hi = 3` wrote all of `pr`. A property is now stored and
selected like a package variable.

The task module (`sim_feature_completion/ki_static_members.rs`) runs every
fixture through the public CLI in both HDL optimizer modes. Expected output was
checked against an independent industry-standard simulator, which gives every
line except those named below. Clause lines are from
`SystemVerilog-1800-2009.txt`.

Common clauses:

- 7.2 L7649-7650: "A structure represents a collection of data types that can
  be referenced as a whole, or the individual data types that make up the
  structure can be referenced by name."
- 8.8 L9869-9870: "Sometimes only one version of a variable is required to be
  shared by all instances. These class properties are created using the keyword
  static." L9887-9888: "Thereafter, every Packet object can access the file
  descriptor in the usual way".
- 6.5 L4647-4649: "Independent elements include different members of a
  structure, or different elements of an array. Each bit in a packed type is
  also an independent element."

## Packed properties

`packed.sv`: members, nested members, bits and parts of packed structures,
unions, enums, packed arrays and vectors, as operands and targets.

## Unpacked properties

`unpacked.sv`: members of unpacked structures (with strings, reals, handles,
arrays and containers in them) and of elements of fixed, dynamic, queue and
associative arrays of them, declared initializers among them. `tagged.sv`: an
unpacked tagged union, which the other simulator does not support.

## Uses

`uses.sv`: members and elements as value, output, inout and `ref` actuals, in
array methods and membership, and as targets of nonblocking assignments,
assignments with a delay or event control and system functions.

## Waits

`waits.sv`: `@`, `wait`, continuous assignments, port actuals, coverpoints and
assertions wake for, and read, the member alone. `implicit.sv`: 9.2.2.2.1
L11184-11185 "The implicit sensitivity list of an always_comb includes the
expansions of the longest static prefix of each variable or select expression
that is read within the block"; a static property is such a variable, so
`always_comb` runs when a member it reads changes and not for another member.
The other simulator leaves static class properties out of `always_comb` and
`always @*`, and refuses one in a sampled value function.

## Scopes

`scopes.sv`: the same members named inside the class (plain, `this.`, a static
method), through `super` and a derived class, a class property, an element of a
container, a virtual interface, a package scope and specializations of a
parameterized class. 8.8 L9893: "The static class properties can be used
without creating an object of that type." 8.24 L10756-10758: "Each
specialization of a class has a separate set of static member variables".

## Containers

`containers.sv`: fixed arrays of queues, dynamic and associative arrays, of
records, nested queues, class handles and events, named through the class, an
object and a null handle of each specialization. 8.8 L9869-9871: "Sometimes
only one version of a variable is required to be shared by all instances. These
class properties are created using the keyword static." 7.4.2 L8053: "Unpacked
arrays can be made of any data type." Stored like a container property of an
object.

## Random properties

`random.sv`: members and elements of static `rand` properties in constraints.
18.6.3 L30468-30470: "Random variables declared as static are shared by all
instances of the class in which they are declared. Each time the randomize()
method is called, the variable is changed in every class instance."
`random_reach.sv`: a static `rand` property of a class whose objects are
reached through a `rand` handle is a variable of the problem, named through the
handle or the class, and is state when no random object of the class is
reached. 18.5.8 L30000-30002: "First, determine the set of objects that are to
be randomized as a whole. Starting with the object that invoked the randomize()
method, add all objects that are contained within it, are declared rand, and
are active (see rand_mode in 18.8)."
`neg_random_record.sv`: a static `rand` property of unpacked structure type is
rejected ([known issues](../../../../../docs/known_issues.md)).

## VPI

`vpi_members.sv`: `vpi_handle_by_name` finds members and elements of static
properties through a class variable and through the class definition, and
reads and writes them; names follow decision
[A19-D87](../../../../../docs/lrm_decisions.md), where the other simulator's
differ.

## Object expressions

The object expression in front of a static property is not needed to find the
property, so llg does not evaluate it. `neg_effects.sv`: one that calls a
subroutine or assigns is rejected, because dropping it would drop its effect.
`neg_interface_queue.sv`: a queue method of a static property through a virtual
interface is rejected ([known issues](../../../../../docs/known_issues.md)).

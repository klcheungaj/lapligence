# Fixed arrays of containers with a range that does not start at 0

A fixed array of queues, dynamic or associative arrays in a class property
(instance or static) is a fixed-size view of the container runtime, whose
storage positions run from 0. Writes of a whole element, writes of an element
of an element and the element loop of a `foreach` over two dimensions took the
index for the position, so `q[0] = '{1, 2}` of `int q[1:0][$]` filled `q[1]`,
while methods and reads mapped it by the declared range. Every path now maps
the index through the one conversion of the view.

The task module (`sim_feature_completion/ki_fixed_view_ranges.rs`) runs the
fixture through the public CLI in both HDL optimizer modes and on every value
backend. Expected output was checked against an independent industry-standard
simulator, which gives every line. Clause lines are from
`SystemVerilog-1800-2009.txt`.

- 7.4.2 L8060-8062: "Each fixed-size dimension shall be represented by an
  address range, such as [1:1024], or a single positive number to specify the
  size of a fixed-size unpacked array, as in C. In other words, [size] becomes
  the same as [0:size-1]."
- 12.7.3 L18219 and L18227-18228: "The foreach-loop arranges for higher
  cardinality indices to change more rapidly." "The second foreach-loop causes
  q to iterate from 5 to 1".
- 21.2.1.7 L36390 and L36402: "For unpacked array data types, it
  shall print the value as an assignment pattern which may include the use of
  index labels."

`containers.sv`: `int q[1:0][$]` (descending, in a method of its class),
`static int s[1:2][$]` (ascending), `static string d[2:1][]` and
`int a[1:2][string]` (through an object): whole-element writes and copies,
element writes, `push_back`, `insert`, `pop_front`, `size`, `num`, an element
passed by value, one- and two-variable `foreach` and `%p` (left index first).

`declarations.sv`: the same arrays declared in a module (`int q[1:0][$]`,
`string d[1:2][]`, `int m[2:1][int]`) and in automatic and static subroutine
storage, which were refused ("unsupported element type", "has no width"):
each element is its own container (7.4.2 L8060).

`real_elements.sv`: reading one real element of such an array (`x[1][0]`, a
module, class, static class and package array) failed the model check
("container expression must produce a packed value", without a position).

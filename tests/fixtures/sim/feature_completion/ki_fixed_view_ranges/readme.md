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

`operands.sv`: an element of such an array (a module's, an automatic one, a
class property's; queues, dynamic and associative arrays of integral, real and
string elements; one and two fixed dimensions) as a whole operand. Equality
of two elements or of an element and a queue variable (also in `if`, `?:` and
`while` conditions), elements as items of an unpacked array concatenation
(also into the same element), and an element passed to a `ref` or
`const ref` formal of a function (also one that reallocates a dynamic array)
were refused ("container element access in tb requires a scalar value", "a
queue or dynamic array nested in a container element as the ref actual of
`x` is not supported"). Each element is read, or written in place, through
an alias of its own storage for the statement.

- 7.4.3 L8089-8095: "The following operations can be performed on all
  arrays, packed or unpacked." ... "Reading and writing an element of the
  array, e.g., A[i] = B[i]" ... "Equality operations on the array or slice of
  the array, e.g., A==B, A[i:j] != B[i:j]".
- 10.10 L13836-13838: "An item whose self-determined type is an unpacked
  array whose slowest-varying dimension's element type is assignment
  compatible with the element type of the target array shall represent as
  many elements as exist in that item".
- 13.5.2 L19103-19104: "Arguments passed by reference are not copied into the
  subroutine area, rather, a reference to the original argument is passed to
  the subroutine."

`neg_task_ref.sv`: the same element passed to a `ref` formal of a task stays
a located error. The alias publishes its changes when the call returns, and a
task can suspend while another process waits on the array.

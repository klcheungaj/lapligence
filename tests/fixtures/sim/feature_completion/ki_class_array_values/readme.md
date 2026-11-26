# Whole uses of fixed arrays that are class properties

A fixed array that is a class property is a fixed-size view of the container
runtime. Uses that read or write it whole took it for fixed storage and were
refused: reductions (`la.sum()`, `o.la.sum()`), ordering methods, a copy
into an automatic array or a return (`t = la`, `return la`), equality with
an array variable, a whole real array through a handle (`t = o.r`,
`o.d = u`, `o.r == t`), and every element use of an instance event-array
property outside its class (`o.ea[0] == a`, `@(o.ea[i])`, `-> o.ea[1]`),
which named the property's own storage instead of the object's and failed
the model check without a source position.

The task module (`sim_feature_completion/ki_class_array_values.rs`) runs every
fixture through the public CLI in both HDL optimizer modes and on every value
backend. Expected output was checked against an independent industry-standard
simulator, which gives every line. Clause lines are from
`SystemVerilog-1800-2009.txt`.

- 7.4 L8088-8094: "The following operations can be performed on all arrays,
  packed or unpacked. [...] Reading and writing the array, e.g., A = B [...]
  Equality operations on the array or slice of the array, e.g., A==B".
- 7.12.2 L9348-9349: "Array ordering methods reorder the elements of any
  unpacked array (fixed or dynamically sized) except for associative arrays."
- 7.12.3 L9393-9394: "Array reduction methods may be applied to any unpacked
  array of integral values to reduce the array to a single value."
- 11.2.2 L14175: "Aggregate expressions can also be compared with equality or
  inequality operators."
- 15.5.5.1 L20862-20863: "When one event variable is assigned to another, the
  two become merged. Thus, executing -> on either event variable affects
  processes waiting on either event variable."

`integral.sv`: `int la[4]` and `byte lb[2:0]`: reductions with and without
`with`, `find`, a return, copies into automatic arrays inside the class and in
a module function, equality both ways (`===` too), `sort`, `rsort` and
`reverse` inside the class and through the handle.

`real_events.sv`: `real r[2]` and `real d[3:1]` copied both ways and compared;
`event ea[2:1]` and `static event sa[1:2]` compared, assigned, waited on with
`@` and `wait (.triggered)` and triggered with `->` and `->>` through handles,
by constant and variable index.

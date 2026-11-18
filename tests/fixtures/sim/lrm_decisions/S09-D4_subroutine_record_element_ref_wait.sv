// Decision S09-D4: a wait on a `ref` formal bound to an element of a member
// array of a task's automatic record resumes when the task that owns the
// record stores into that element; a store into another element or member
// does not resume it.
//
// IEEE 1800-2009 13.5.2 (SystemVerilog-1800-2009.txt L19138-19141): "When the
//   argument is passed by reference, both the caller and the subroutine share
//   the same representation of the argument; therefore, any changes made to
//   the argument, within either the caller or the subroutine, shall be
//   visible to each other."
// IEEE 1800-2009 13.5.2 (L19143-19147): "Only the following shall be legal
//   to pass by reference: a variable, a class property, a member of an
//   unpacked structure, or an element of an unpacked array."
// IEEE 1800-2009 9.4.3 (L12032-12033): "The wait statement shall evaluate a
//   condition; and, if it is false, the procedural statements following the
//   wait statement shall remain blocked until that condition becomes true".
//
// llg follows the text (these waits never resumed before). Some simulators
// resume such a wait only for a whole variable, never for an element.
`timescale 1ns/1ns
module tb;
  typedef struct { string s; int n; } el_t;
  typedef struct { el_t e[2]; int k; } rec_t;

  task automatic on_element(ref el_t r, input string tag, input int n);
    wait (r.n == n);
    $display("%s n=%0d at %0d", tag, r.n, $time);
  endtask

  task automatic owner();
    rec_t t;
    fork
      on_element(t.e[1], "e1", 2);
      on_element(t.e[0], "e0", 3);
    join_none
    #1 t.e[0].n = 2;
    t.k = 2;
    #1 t.e[1].n = 2;
    #1 t.e[1].n = 3;
    #1 t.e[0].n = 3;
    #1 $display("done at %0d", $time);
  endtask

  initial begin
    owner();
    $finish;
  end
endmodule

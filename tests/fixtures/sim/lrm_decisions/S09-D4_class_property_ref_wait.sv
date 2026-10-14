// Decision S09-D4: a wait on a `ref` formal bound to a class property, or to
// an element of an unpacked array, resumes when that property or element
// changes; a change of another element does not resume it.
//
// IEEE 1800-2009 13.5.2 (SystemVerilog-1800-2009.txt L19143-19147): "Only the
//   following shall be legal to pass by reference: a variable, a class
//   property, a member of an unpacked structure, or an element of an unpacked
//   array."
// IEEE 1800-2009 13.5.2 (L19140-19141): "The semantics of assignments to
//   variables passed by reference is that changes are seen outside the
//   subroutine immediately (before the subroutine returns)."
// IEEE 1800-2009 9.4.3 (L12032-12033): "The wait statement shall evaluate a
//   condition; and, if it is false, the procedural statements following the
//   wait statement shall remain blocked until that condition becomes true".
//
// llg follows the text (it never woke a task waiting on a class property
// before).
module tb;
  class Counter;
    int x;
  endclass
  Counter c;
  int arr[3];

  task automatic reach(ref int r, input string tag, input int n);
    wait (r >= n);
    $display("%s %0d at %0d", tag, n, $time);
    if (n < 2) reach(r, tag, n + 1);
  endtask

  initial begin
    c = new;
    fork
      reach(c.x, "property", 1);
      reach(arr[1], "element", 1);
    join_none
    #1 c.x = 1;
    #1 arr[0] = 5;
    #1 arr[1] = 2;
    #1 c.x = 2;
    #1 $finish;
  end
endmodule

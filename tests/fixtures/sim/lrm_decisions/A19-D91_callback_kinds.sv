// Decision A19-D91: callback kinds llg refuses.
// - cbSizeChange is refused with an error on a dynamic array, a queue, an
//   associative array and a string var, in every value format.
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L64657): "cbSizeChange
// After a dynamic array, associative array, queue, or string has been
// resized." 37.17 detail 14 (L58772-58774): "cbSizeChange shall be
// applicable only for dynamic and associative arrays, for queues, and for
// string vars. ... The value in the callback is the new size of the array."
// Industry-standard behaviour refuses cbSizeChange on a string var and on
// any value format but vpiSuppressVal, and accepts a vpiSuppressVal record
// on a container that is never called when the container is resized. llg
// refuses every registration, so that an application learns the callback
// will not occur.
//
// Build A19-D91_callback_kinds.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d91(input int)` declared as a system task).
`timescale 1ns/1ns
module tb;
  int da[];
  int q[$];
  int aa[int];
  string s;

  initial begin
    $d91(0);
    #1 da = new[3];
    q.push_back(1);
    aa[5] = 1;
    s = "abc";
    #1 $display("done %0d %0d %0d %0d", da.size(), q.size(), aa.num(), s.len());
    $finish;
  end
endmodule

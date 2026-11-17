// Decision A19-D91: callback kinds llg refuses.
// - cbSizeChange is refused with an error on a dynamic array, a queue, an
//   associative array and a string var, in every value format.
// - cbValueChange is refused with an error on a continuous assignment and
//   on a gate.
// - cbValueChange on a chandle variable is accepted except with the
//   vpiStrengthVal format, which is refused with an error.
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
// 38.36.1 (L64642-64644): "cbValueChange After value change on some
// variables, any expression, or terminal or after execution of an event
// statement." A continuous assignment and a primitive are none of these.
// Industry-standard behaviour refuses a gate with an error and returns NULL
// for a continuous assignment without one; llg reports an error for both
// (an application can watch the net they drive).
// Table 38-5 (L64735): "vpiStrengthVal Value plus strength information of
// a scalar object only"; a chandle is not scalar. Industry-standard
// behaviour accepts the record and passes it no strength value.
//
// Build A19-D91_callback_kinds.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d91(input int)` declared as a system task).
`timescale 1ns/1ns
module tb;
  int da[];
  int q[$];
  int aa[int];
  string s;
  chandle h;
  logic a, b;
  wire w, y;
  assign w = a & b;
  and g1(y, a, b);

  initial begin
    $d91(0);
    #1 da = new[3];
    q.push_back(1);
    aa[5] = 1;
    s = "abc";
    h = null;
    a = 1'b1;
    b = 1'b1;
    #1 $display("done %0d %0d %0d %0d %b %b %0d", da.size(), q.size(), aa.num(), s.len(), w, y,
                h == null);
    $finish;
  end
endmodule

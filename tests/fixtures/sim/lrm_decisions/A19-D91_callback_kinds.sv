// Decision A19-D91: cbSizeChange, and the callback kinds llg refuses.
// - cbSizeChange with vpiSuppressVal is served on a dynamic array, a queue
//   and an associative array variable of a scope: it runs after each resize
//   (each push and each pop, a new[] of a new size, an insertion or deletion
//   of a key, delete), not after an element write or a new[] of the same
//   size, and passes no value. It is refused with an error on a string var,
//   with another value format and on a class property. A removed record is
//   not called again.
// - cbValueChange is refused with an error on a continuous assignment and
//   on a gate.
// - cbValueChange on a chandle variable is accepted except with the
//   vpiStrengthVal format, which is refused with an error.
//
// IEEE 1800-2009 38.36.1 (SystemVerilog-1800-2009.txt L64657): "cbSizeChange
// After a dynamic array, associative array, queue, or string has been
// resized." 37.17 detail 14 (L58772-58774): "cbSizeChange shall be
// applicable only for dynamic and associative arrays, for queues, and for
// string vars. If both value and size change, the size change callback
// shall be invoked first. This callback fires after the size change occurs
// and before any value changes for that variable. The value in the callback
// is the new size of the array." Industry-standard behaviour accepts the
// registrations llg serves, with vpiSuppressVal only (no value is passed),
// and refuses a string var and the other formats; llg also refuses the
// containers whose resizes it cannot watch, so that no accepted record
// stays silent.
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
  class C;
    int d[$];
  endclass
  int da[];
  int q[$];
  int aa[int];
  C c = new;
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
    #1 da = new[3];
    #1 da = new[5](da);
    #1 da[1] = 7;
    #1 begin q.push_back(2); q.push_back(3); end
    #1 begin q.push_back(4); void'(q.pop_back()); end
    #1 q = '{5, 6, 7, 8};
    #1 aa[5] = 2;
    #1 aa.delete(5);
    #1 c.d.push_back(1);
    #1 $d91(1);
    #1 da.delete();
    #1 q.delete();
    #1 $display("end %0d %0d %0d", da.size(), q.size(), aa.num());
    $finish;
  end
endmodule

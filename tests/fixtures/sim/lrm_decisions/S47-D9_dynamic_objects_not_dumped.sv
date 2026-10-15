// Decision S47-D9: VCD/FST dumps hold packed and real variables and nets,
// fixed unpacked arrays of them and unpacked structure members declared in
// instance and generate scopes. Strings, class handles, events, dynamic,
// queue and associative arrays, automatic variables and variables declared
// in blocks or subroutines are left out of a scope dump; naming one of them
// in $dumpvars is a compile error (case S47-D9_named_string_rejected).
//
// IEEE 1800-2009 21.7.2.1 (SystemVerilog-1800-2009.txt L38065-38066): "Value
// changes for all other variables are specified in binary format by 0, 1, x,
// or z values." Syntax 21-20 (L38022-38031) has no value format for strings, handles or
// containers. The text does not say what $dumpvars does with them.
// Compare S47-D9.vcd: it declares tb.count only.
class Item;
  int value;
endclass

module tb;
  logic [3:0] count;
  string label;
  Item item;
  int q[$];
  initial begin
    count = 4'd0;
    $dumpfile("S47-D9.vcd");
    $dumpvars(0, tb);
    #1 count = 4'd1;
    label = "one";
    item = new;
    item.value = 5;
    q.push_back(7);
    $display("time 1: count=%0d label=%s value=%0d size=%0d", count, label, item.value,
             q.size());
    #1 item = null;
    $finish;
  end
endmodule

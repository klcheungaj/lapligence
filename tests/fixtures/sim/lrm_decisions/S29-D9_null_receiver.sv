// IEEE 1800-2009 18.6.1 L30392-30400 describes randomize() as a method of an
// object; the text does not say what calling it through a null handle does.
// Decision (llg choice, the rule for every method call through a null
// handle): it is a run-time error at the call. The run stops with exit
// status 1 after the first line.
class item_c;
  rand bit [3:0] a;
endclass
module tb;
  initial begin
    item_c item;
    int ok;
    $display("before");
    ok = item.randomize();
    $display("after ok=%0d", ok);
    $finish;
  end
endmodule

// Decision S39-D3: simple immediate assertions obey $assertoff/$asserton and
// the action-control tasks like every other assertion.
//
// IEEE 1800-2009 20.11 (SystemVerilog-1800-2009.txt L35048): "$assertoff
// shall stop the checking of all specified assertions until a subsequent
// $asserton"; 20.12 L35122-35123: "$assertfailoff shall stop execution of the
// fail action of all the specified assertions"; 16.3 L21073-21074: "the tool
// shall, by default, call $error, unless $assertfailoff is used to suppress
// the failure". (llg formerly ignored the tasks for simple immediate
// assertions.)
`timescale 1ns / 1ns
module tb;
  initial begin
    $assertoff;
    assert (1'b0) else $display("%0t off fail", $time);
    #1 $asserton;
    assert (1'b0) else $display("%0t on fail", $time);
    #1 $assertfailoff;
    assert (1'b0) else $display("%0t failoff fail", $time);
    #1 $assertfailon;
    assert (1'b1) $display("%0t pass", $time);
    #1 $assertpassoff;
    assert (1'b1) $display("%0t passoff pass", $time);
    #1 $finish;
  end
endmodule

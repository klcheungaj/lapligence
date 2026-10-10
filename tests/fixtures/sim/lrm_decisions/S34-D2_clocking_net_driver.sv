// Decision S34-D2: a clocking output or inout whose signal is a net drives it
// through a driver of its own, which starts at 'z and resolves with the net's
// other drivers; a synchronous drive never overwrites another driver.
//
// IEEE 1800-2009 14.16 (SystemVerilog-1800-2009.txt L20069-20074):
//   "For each clocking block output whose target is a net, a driver on that
//   net shall be created. The driver so created shall have (strong1, strong0)
//   drive strength and shall be updated as if by a continuous assignment from
//   a variable inside the clocking block. This implicit variable, which is
//   invisible to user code, shall be updated in the Re-NBA region by the
//   execution of a synchronous drive to the corresponding clockvar. The
//   created driver shall be initialized to 'z, hence, the driver has no
//   influence on its target net until a synchronous drive is performed to the
//   corresponding clockvar."
//
// llg wrote a drive into the net's existing driver before.
module tb;
  bit clk = 0;
  wire [3:0] w;
  logic [3:0] other = 4'b0001;
  assign w = other;
  clocking cb @(posedge clk);
    inout w;
  endclocking
  always #5 clk = ~clk;
  initial begin
    #1 $display("%0d before any drive w=%b", $time, w);
    @(posedge clk);
    cb.w <= 4'b0011;
    #1 $display("%0d both drivers w=%b", $time, w);
    other = 4'bzzzz;
    #1 $display("%0d clocking driver only w=%b", $time, w);
    $finish;
  end
endmodule

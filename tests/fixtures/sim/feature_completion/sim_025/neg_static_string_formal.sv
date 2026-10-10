// SIM-025 limitation: a static task keeps its string formal in the call
// activation, which a deferred report cannot read. Packed and real static
// formals and locals work (static_subroutines.sv).
module tb;
  task static t(input string s);
    $strobe("s=%s", s);
  endtask
  initial begin
    t("x");
    #1 $finish(0);
  end
endmodule

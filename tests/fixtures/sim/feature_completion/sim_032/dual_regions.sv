// SIM-032 A01: Active versus Reactive, Inactive versus Re-Inactive and NBA
// versus Re-NBA in one time slot (IEEE 1800-2009 4.4.2, 4.5, 24.3.1).
module tb;
  logic m = 1'b0, p = 1'b0;
  initial begin
    m <= 1'b1;
    $display("A0 m=%0d p=%0d", m, p);
    #0 $display("A1 m=%0d p=%0d", m, p);
  end
  always @(p) $display("A2 module saw p=%0d", p);
  program pr;
    initial begin
      $display("R0 m=%0d p=%0d", m, p);
      p <= 1'b1;
      #0 $display("R1 m=%0d p=%0d", m, p);
      #0 $display("R2 m=%0d p=%0d", m, p);
      #1 $display("R3 m=%0d p=%0d", m, p);
    end
  endprogram
endmodule

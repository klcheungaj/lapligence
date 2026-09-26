// IEEE 1800-2009 7.12.2 adds unpacked-array reverse to Verilog-2001.
module tb;
  reg [7:0] values [0:1];
  initial begin
    values[0] = 8'd1;
    values[1] = 8'd2;
    values.reverse();
    if (values[0] !== 8'd2 || values[1] !== 8'd1) begin
      $display("FAIL reverse order");
      $finish;
    end
    $display("PASS syn027_reverse_edition");
    $finish;
  end
endmodule

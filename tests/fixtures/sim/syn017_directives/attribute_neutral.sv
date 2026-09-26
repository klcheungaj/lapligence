// IEEE 1364-2001 2.8: attributes are metadata, not dataflow drivers.
module tb;
  (* keep = 1 *) wire [3:0] value;
  assign value = 4'ha;
  initial begin
    #0;
    $display("attribute=%h", value);
    $finish;
  end
endmodule

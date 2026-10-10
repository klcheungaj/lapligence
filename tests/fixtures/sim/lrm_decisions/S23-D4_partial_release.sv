// Decision S23-D4: forces and releases on a net act bit by bit. A later force
// of an overlapping constant part-select replaces only the overlapped bits of
// an earlier force, and releasing a part of a forced range releases only
// those bits; the rest stay forced.
//
// IEEE 1800-2009 10.6.2 (SystemVerilog-1800-2009.txt L13373-13374) admits "a
// constant bit-select of a vector net, a constant part-select of a vector
// net, or a concatenation of these" as targets, and (L13397-13399): "A force
// procedural statement on a net shall override all drivers of the net ...
// until a release procedural statement is executed on the net. When
// released, the net shall immediately be assigned the value determined by the
// drivers of the net." The text does not say how forces and releases of
// overlapping selects combine.
// IEEE 1800-2009 4.9.2 (L3519): "A deassign or a release statement
// deactivates any corresponding assign or force statement(s)."
//
// llg's choice: each bit belongs to the most recent force that covered it,
// and a release affects exactly the bits it names.
module tb;
  logic [7:0] d;
  wire [7:0] n;
  assign n = d;
  initial begin
    d = 8'h00;
    force n[7:4] = 4'hf;
    #1 $display("1 n=%b", n);
    force n[5:2] = 4'b0110;
    #1 $display("2 n=%b", n);
    release n[3:2];
    #1 $display("3 n=%b", n);
    d = 8'hff;
    #1 $display("4 n=%b", n);
    release n[7];
    #1 $display("5 n=%b", n);
    release n;
    #1 $display("6 n=%b", n);
    $finish;
  end
endmodule

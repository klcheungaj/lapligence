// ADV-005: MOS, CMOS and resistive MOS switches stay unsupported (ADV-004).
module tb;
  wire o, i;
  reg c;
  nmos n1(o, i, c);
  initial $finish;
endmodule

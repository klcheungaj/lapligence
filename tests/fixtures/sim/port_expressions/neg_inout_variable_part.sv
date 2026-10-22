// A concatenated port with an inout part may contain only nets.
module m(.p({a, b}));
  inout a;
  output logic b;
endmodule

module tb;
  wire [1:0] w;
  m c(.p(w));
endmodule

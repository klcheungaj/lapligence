// Adopted FND-002 witness neg_udp_bad_row (L-F08-09-01).
// SV2009 29.3: every table row has one field per input.
// Expected: required diagnostic
primitive p(q,a); output q; input a; table 0 1:0; endtable endprimitive
module tb; wire q; p u(q,0); initial $finish; endmodule

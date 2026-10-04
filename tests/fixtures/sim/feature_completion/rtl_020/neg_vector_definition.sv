// Adopted FND-002 witness neg_udp_vector (L-F08-09-02).
// SV2009 29.2,29.3: UDP definition ports are scalar; a vector port is illegal.
// Expected: required diagnostic
primitive p(q,a); output q; input [1:0] a; table 0:0; 1:1; endtable endprimitive
module tb; wire q; p u(q,2'b10); initial $finish; endmodule

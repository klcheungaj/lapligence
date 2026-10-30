// Decision A19-D35: drive strengths and the declaration properties of nets
// and ports (IEEE 1800-2009 37.14, 37.16, 37.31, 37.43).
// vpiStrength0 and vpiStrength1 of a primitive or continuous assignment
// are its drive strengths (vpiSupplyDrive ... vpiHiZ); without a
// specification a gate, switch and continuous assignment drive strong
// (10.3.4, 28.3.2) and a pullup or pulldown source pull (28.10, "If no
// strength specification is given, then the default strength shall be
// pull"). A net reports 0 for both: llg does not model charge strength
// (trireg) and a net's own drivers carry the strengths. vpiNetDeclAssign
// is 1 for a net declaration assignment and for the net (or net bit) it
// assigns. vpiExplicitScalared and vpiExplicitVectored report the
// `scalared` and `vectored` keywords; vpiExpanded is 1 only for a
// `scalared` net (6.9.2: "If the keyword scalared is used, ... the PLI
// shall consider the net expanded"; llg keeps every other vector whole),
// and a net bit reports its parent's (detail 18). vpiImplicitDecl
// is 1 for an implicitly declared net. vpiResolvedNetType is the type of
// the simulated net: the net's own type, unless a port joins it to a net
// whose type dominates by Table 23-1 (23.3.3.7). vpiExplicitName is 1 for a
// port declared as `.name(expression)`; vpiConnByName is 1 for a port that
// the instance connects by name, `.name` and `.*` included.
module pulled (input tri0 ti);
endmodule
module named (.p(x), z);
  input x;
  input z;
endmodule
module inner (input wire wi, output wire wo);
  assign wo = wi;
endmodule
module tb;
  wire a = 1'b1;
  wire (pull0, weak1) b = a;
  wire c, d, e, f, h, t, ext;
  wire scalared [3:0] sc;
  wire vectored [3:0] vc;
  supply0 s0;
  triand ta;
  tri tt;
  wire wi = a;
  wire wo;
  assign sc = 4'h1;
  assign vc = 4'h2;
  assign (weak0, strong1) c = a;
  assign ta = a;
  assign tt = a;
  and (weak0, weak1) g1 (d, a, a);
  and g2 (e, a, a);
  pullup (f);
  pulldown (strong0) pd (h);
  tran sw (t, h);
  buf ib (imp, a);
  pulled p1 (ext);
  named n1 (.p(a), .z(a));
  named n2 (a, a);
  inner i1 (.*);
  inner i2 (.wi, .wo());
  initial begin
    #1 $d35;
    $display("c=%b d=%b f=%b imp=%b", c, d, f, imp);
    $finish;
  end
endmodule

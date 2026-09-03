// llg-lsp-fixture: root-a/navigation/foo.v

module Bar(input i, output o);
assign o = i;
endmodule

module Foo();

wire w1;
wire w2;
wire w3;

Bar Bar(
    .i(w1),
    .o(w2)
);

Bar u_bar(
    .i(w2),
    .o(w3)
);

endmodule

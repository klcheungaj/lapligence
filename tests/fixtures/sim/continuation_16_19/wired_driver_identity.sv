module wired_identity_leaf(input wire [64:0] value);
    wand [64:0] a;
    wor [64:0] o;
    assign a=value;
    assign o=value;
endmodule
module tb;
    reg [64:0] p, q, r;
    wired_identity_leaf first(p), second(q);
    assign first.a=q;
    assign first.a=r;
    assign first.o=q;
    assign first.o=r;
    assign second.a=p;
    assign second.a=r;
    assign second.o=p;
    assign second.o=r;
endmodule

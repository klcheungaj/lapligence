module child(input wire a, inout wand wa, inout wor wo);
    assign wa = a;
    assign wo = a;
endmodule
module tb;
    logic a, b;
    wand wa;
    wor wo;
    assign wa = b;
    assign wo = b;
    child c(.a(a), .wa(wa), .wo(wo));
    initial begin
        a = 1; b = 1;
        #1; $display("resolved=%b%b", wa, wo);
        a = 0; b = 1;
        #1; $display("resolved=%b%b", wa, wo);
        a = 1'bz; b = 1'bz;
        #1; $display("resolved=%b%b", wa, wo);
        a = 1'bx; b = 1;
        #1; $display("resolved=%b%b", wa, wo);
        a = 0; b = 1'bx;
        #1; $display("resolved=%b%b", wa, wo);
        $finish(0);
    end
endmodule

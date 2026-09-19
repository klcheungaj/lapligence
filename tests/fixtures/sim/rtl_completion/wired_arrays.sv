module tb;
    logic [7:0] a, b;
    wand [7:0] ands[1:-1];
    wor [7:0] ors[2];
    tri0 [7:0] zeros[2];
    tri1 ones[2];
    assign ands[0] = a;
    assign ands[0] = b;
    assign ors[1][7:4] = a[7:4];
    assign ors[1][7:4] = b[7:4];
    assign ors[1][3:0] = 4'h5;
    initial begin
        a=8'hf0; b=8'haa;
        #1;
        $display("and=%h other=%h or=%h defaults=%h,%b", ands[0], ands[1], ors[1], zeros[0], ones[1]);
        a=8'h0f; b=8'h33;
        #1;
        $display("and=%h other=%h or=%h defaults=%h,%b", ands[0], ands[-1], ors[1], zeros[1], ones[0]);
        $finish(0);
    end
endmodule

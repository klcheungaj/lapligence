interface bus;
    wand [7:0] a;
    wor [7:0] o;
endinterface
module tb;
    bus first();
    bus second();
    logic [7:0] left, right;
    assign first.a = left;
    assign first.a = right;
    assign first.o = left;
    assign first.o = right;
    assign second.a = 8'haa;
    initial begin
        left = 8'hf0; right = 8'h0f;
        #1 $display("a=%h b=%h o=%h", first.a, second.a, first.o);
        right = 8'hf0;
        #1 $display("a=%h b=%h o=%h", first.a, second.a, first.o);
        $finish(0);
    end
endmodule

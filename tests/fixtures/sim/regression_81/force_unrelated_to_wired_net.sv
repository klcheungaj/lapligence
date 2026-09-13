module tb;
    wand w;
    logic source = 0;
    logic value = 0;
    assign w = 1'b1;
    initial begin
        force value = source;
        source = 1;
        #1;
        $display("forced=%b wired=%b", value, w);
        release value;
        source = 0;
        #1;
        $display("released=%b wired=%b", value, w);
        value = 0;
        $display("assigned=%b", value);
        $finish(0);
    end
endmodule

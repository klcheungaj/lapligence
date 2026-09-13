module tb;
    logic [3:0] a=0;
    logic en=1;
    int rises,filtered,falls;
    always @(posedge a) rises=rises+1;
    always @(posedge (a | 4'b0000) iff en) filtered=filtered+1;
    always @(negedge a) falls=falls+1;
    initial begin
        #1 a=4'b1000;
        #1 a=4'b1001;
        #1 a=4'bxxx1;
        #1 a=4'bxxx0;
        #1 a=4'bxxxz;
        #1 a=4'bxxx1;
        #1 $display("%0d %0d %0d",rises,filtered,falls);
        $finish(0);
    end
endmodule

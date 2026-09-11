module tb;
    logic clk,en;
    int count;
    always @(posedge clk iff en) count=count+1;
    initial begin
        clk=0; en=0;
        #1 clk=1; en=1;
        #1 $display("rejected %0d",count);
        clk=0;
        #1 clk=1; en=0;
        #1 $display("accepted %0d",count);
        clk=0; en=1'bx;
        #1 clk=1;
        #1 $display("unknown %0d",count);
        $finish;
    end
endmodule

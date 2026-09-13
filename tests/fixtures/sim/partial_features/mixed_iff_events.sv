module tb;
    logic clk,en;
    event ev;
    int count;
    always @(posedge clk iff en or ev iff !en) count=count+1;
    initial begin
        clk=0; en=1;
        #1 ->ev;
        #1 $display("filtered %0d",count);
        clk=1;
        #1 $display("clock %0d",count);
        en=0; ->ev;
        #1 $display("event %0d",count);
        en=1; ->ev;
        #1 $display("filtered %0d",count);
        $finish(0);
    end
endmodule

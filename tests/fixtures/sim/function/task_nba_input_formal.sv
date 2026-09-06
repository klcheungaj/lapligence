module tb;
    task schedule_input(input logic [7:0] value);
        value <= 8'h5a;
    endtask

    initial begin
        schedule_input(8'h00);
        #1 $finish;
    end
endmodule

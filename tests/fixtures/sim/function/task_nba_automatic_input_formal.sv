// IEEE 1800-2009 10.4.2 and 13.3.2 prohibit an NBA to automatic storage.
module tb;
    task automatic schedule_input(input logic [7:0] value);
        value <= 8'h5a;
    endtask

    initial begin
        schedule_input(8'h00);
        #1 $finish;
    end
endmodule

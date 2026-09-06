// IEEE 1800-2009 10.4.2 and 13.3.2 prohibit an NBA to automatic storage.
module tb;
    task automatic schedule_local(input logic [7:0] next_value);
        logic [7:0] stored;
        stored <= next_value;
    endtask

    initial begin
        schedule_local(8'h5a);
        #1 $finish;
    end
endmodule

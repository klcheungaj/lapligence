module tb;
    logic [7:0] values [0:1];
    integer index_calls;
    logic [7:0] observed;

    function automatic integer next_index();
        next_index = 0;
        index_calls = index_calls + 1;
    endfunction

    task add_one(input logic [7:0] amount, inout logic [7:0] value);
        value = value + amount;
    endtask

    initial begin
        values[0] = 4;
        values[1] = 99;
        index_calls = 0;
        add_one(8'd1, values[next_index()]);
        observed = values[0];
        $display("value=%0d index_calls=%0d", observed, index_calls);
        $finish;
    end
endmodule

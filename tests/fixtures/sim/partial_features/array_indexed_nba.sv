`timescale 1ns/1ns
module tb;
    logic [7:0] memory[0:1];
    int element_index, base_index, delay_value;
    int calls = 0;
    function automatic int selected_base();
        calls = calls + 1;
        return base_index;
    endfunction
    initial begin
        memory[0] = 0;
        memory[1] = 0;
        element_index = 0;
        base_index = 0;
        delay_value = 2;
        memory[element_index][selected_base() +: 4] <= #delay_value 4'h9;
        memory[0][7 -: 1] <= #delay_value 1'b1;
        element_index = 1;
        base_index = 7;
        delay_value = 1;
        memory[element_index][selected_base() -: 4] <= #delay_value 4'ha;
        $display("issued %0d %h %h %0d", $time, memory[0], memory[1], calls);
        #1 memory[0][4 +: 3] = 3'b101;
        #0 $strobe("early %0d %h %h %0d", $time, memory[0], memory[1], calls);
        #1 $strobe("late %0d %h %h %0d", $time, memory[0], memory[1], calls);
        #1 $finish;
    end
endmodule

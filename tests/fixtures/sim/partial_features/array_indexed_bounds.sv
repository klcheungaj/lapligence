module tb;
    logic [7:0] memory[-1:0];
    logic [128:0] wide[0:0];
    logic [128:0] base_index;
    int element_index;
    initial begin
        memory[0] = 8'ha5;
        element_index = 0;
        base_index = '0;
        base_index[100] = 1;
        memory[element_index][base_index +: 4] = '1;
        base_index = 'x;
        memory[element_index][base_index -: 4] <= '0;
        memory[2][0 +: 4] <= '0;
        memory[0][6 +: 4] = 4'b0011;
        memory[0][-2 +: 4] <= 4'b1100;
        wide[0] = '0;
        wide[0][63 +: 66] = '1;
        #1;
        $display("bounds %h %b %b", memory[0], memory[0][6 +: 4], memory[0][-2 +: 4]);
        $display("wide %0d %b %b", $countones(wide[0]), wide[0][128], wide[0][62]);
        $finish(0);
    end
endmodule

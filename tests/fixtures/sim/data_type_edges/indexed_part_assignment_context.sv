// IEEE 1800-2009 11.5.1, 11.6, and 11.8.2: an indexed part-select
// supplies its fixed width to the assignment context, and writes to a
// partially out-of-range select affect only the underlying in-range bits.
module tb #(parameter WIDTH = 128);
    logic [7:0] add_left;
    logic [7:0] add_right;
    logic [127:0] wide_target;
    logic [7:0] small_target;

    initial begin
        add_left = 8'd255;
        add_right = 8'd1;
        wide_target = '1;
        wide_target[0 +: 128] = add_left + add_right;
        if (wide_target !== 128'd256) begin
            $display("FAIL indexed_part_assignment_context addition WIDTH=%0d", WIDTH);
            $finish;
        end

        small_target = 8'ha5;
        small_target[4 +: 96] = 96'h00000000000000000000000c;
        if (small_target !== 8'hc5) begin
            $display("FAIL indexed_part_assignment_context clipped_write WIDTH=%0d",
                     WIDTH);
            $finish;
        end

        $display("PASS indexed_part_assignment_context WIDTH=%0d", WIDTH);
        $finish;
    end
endmodule

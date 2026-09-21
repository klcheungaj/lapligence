// IEEE 1800-2009 7.12.2: fixed-array reverse preserves declaration-order
// elements, including reversed and negative unpacked ranges.
module tb;
    typedef struct packed {
        logic [7:0] tag;
        logic [7:0] value;
    } record_t;

    logic [7:0] one [0:0];
    logic [7:0] two [2:1];
    byte three [0:2];
    logic [7:0] seventeen [16:0];
    logic [7:0] negative [-2:2];
    record_t records [0:2];
    logic [7:0] rows [0:1][2:0];
    byte formal_values [0:2];

    task automatic reverse_local;
        byte local_values [0:2];
        local_values[0] = 8'd4;
        local_values[1] = 8'd5;
        local_values[2] = 8'd6;
        local_values.reverse();
        local_values.reverse();
        if (local_values[0] !== 8'd4
                || local_values[1] !== 8'd5
                || local_values[2] !== 8'd6) begin
            $display("FAIL syn_027_reverse_local");
            $finish;
        end
    endtask

    task automatic reverse_ref(ref byte values [0:2]);
        values.reverse();
    endtask

    initial begin
        one[0] = 8'd1;
        one.reverse();
        if (one[0] !== 8'd1) begin
            $display("FAIL syn_027_reverse_one");
            $finish;
        end

        two[2] = 8'd2;
        two[1] = 8'd1;
        two.reverse();
        if (two[2] !== 8'd1 || two[1] !== 8'd2) begin
            $display("FAIL syn_027_reverse_two");
            $finish;
        end

        three[0] = 8'd10;
        three[1] = 8'd11;
        three[2] = 8'd12;
        three.reverse();
        if (three[0] !== 8'd12 || three[1] !== 8'd11 || three[2] !== 8'd10) begin
            $display("FAIL syn_027_reverse_three");
            $finish;
        end

        for (int index = 0; index <= 16; index++) begin
            seventeen[index] = index + 8'd32;
        end
        seventeen.reverse();
        if (seventeen[16] !== 8'd32 || seventeen[8] !== 8'd40
                || seventeen[0] !== 8'd48) begin
            $display("FAIL syn_027_reverse_seventeen");
            $finish;
        end

        for (int index = -2; index <= 2; index++) begin
            negative[index] = index + 8'd20;
        end
        negative.reverse();
        if (negative[-2] !== 8'd22 || negative[0] !== 8'd20
                || negative[2] !== 8'd18) begin
            $display("FAIL syn_027_reverse_negative");
            $finish;
        end

        records[0].tag = 8'h10;
        records[0].value = 8'ha0;
        records[1].tag = 8'h11;
        records[1].value = 8'ha1;
        records[2].tag = 8'h12;
        records[2].value = 8'ha2;
        records.reverse();
        if (records[0].tag !== 8'h12 || records[0].value !== 8'ha2
                || records[2].tag !== 8'h10 || records[2].value !== 8'ha0) begin
            $display("FAIL syn_027_reverse_records");
            $finish;
        end

        rows[0][2] = 8'd1;
        rows[0][1] = 8'd2;
        rows[0][0] = 8'd3;
        rows[1][2] = 8'd4;
        rows[1][1] = 8'd5;
        rows[1][0] = 8'd6;
        rows[1].reverse();
        if (rows[1][2] !== 8'd6 || rows[1][1] !== 8'd5 || rows[1][0] !== 8'd4
                || rows[0][2] !== 8'd1 || rows[0][0] !== 8'd3) begin
            $display("FAIL syn_027_reverse_selected_row");
            $finish;
        end

        reverse_local();

        formal_values[0] = 8'd7;
        formal_values[1] = 8'd8;
        formal_values[2] = 8'd9;
        reverse_ref(formal_values);
        if (formal_values[0] !== 8'd9
                || formal_values[1] !== 8'd8
                || formal_values[2] !== 8'd7) begin
            $display("FAIL syn_027_reverse_ref");
            $finish;
        end

        $display("PASS syn_027_fixed_reverse");
        $finish;
    end
endmodule

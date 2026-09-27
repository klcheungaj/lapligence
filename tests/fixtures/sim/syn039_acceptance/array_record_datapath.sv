// llg-test-fixture: SYN-039 array-of-record datapath.
// IEEE 1800-2009 §§7.2, 7.4.2, 9.2.2.2, 13.4.2, 23.2.2.2.
// A fixed array of unpacked records crosses a value port, is read by a
// zero-time function from combinational logic, and is captured by a clocked
// process. The record array is kept as an actual source value in the parent.
typedef struct {
    logic [7:0] tag;
    logic [7:0] value;
} record_t;

module record_stage #(
    parameter int N = 2
) (
    input logic clk,
    input logic reset_n,
    input record_t in_records [0:N-1],
    output record_t out_records [0:N-1],
    output logic [8:0] total
);
    logic [8:0] combinational_total;

    function automatic logic [8:0] score(input record_t item);
        score = item.tag + item.value;
    endfunction

    always_comb begin
        combinational_total = '0;
        for (int i = 0; i < N; i = i + 1) begin
            out_records[i].tag = in_records[i].tag + 8'd1;
            out_records[i].value = in_records[i].value + score(in_records[i]);
            combinational_total = combinational_total + score(in_records[i]);
        end
    end

    always_ff @(posedge clk) begin
        if (!reset_n)
            total <= '0;
        else
            total <= combinational_total;
    end
endmodule

module tb;
    record_t input_records [0:1];
    record_t output_records [0:1];
    logic clk;
    logic reset_n;
    logic [8:0] total;
    integer seed;

    record_stage #(.N(2)) dut (
        .clk(clk),
        .reset_n(reset_n),
        .in_records(input_records),
        .out_records(output_records),
        .total(total)
    );

    initial begin
        if (!$value$plusargs("seed=%d", seed))
            $fatal(1, "missing runtime seed");
        clk = 1'b0;
        reset_n = 1'b0;
        input_records[0] = '{tag: 8'd3, value: 8'(4 + seed)};
        input_records[1] = '{tag: 8'd5, value: 8'(6 + seed)};
        #1;
        if (output_records[0].tag !== 8'd4
                || output_records[0].value !== 8'(11 + 2 * seed)
                || output_records[1].tag !== 8'd6
                || output_records[1].value !== 8'(17 + 2 * seed))
            $fatal(1, "array record combinational result");
        clk = 1'b1;
        #1 clk = 1'b0;
        reset_n = 1'b1;
        #1 clk = 1'b1;
        #1 clk = 1'b0;
        $display("records=%0d/%0d total=%0d", output_records[0].value,
                 output_records[1].value, total);
        $finish(0);
    end
endmodule

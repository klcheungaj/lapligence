// IEEE 1800-2009 11.9: a select of an inactive member reads or writes that
// member, so it is a run-time error. Reads yield X; writes store nothing.
// Selectors are still evaluated once. Results go to stderr with the reports.
typedef union tagged packed {
    logic [7:0] A;
    logic [7:0] B;
} packed_t;

typedef union tagged {
    int Count;
    logic [7:0] Row [0:1];
} unpacked_t;

module tb;
    localparam logic [31:0] STDERR = 32'h8000_0002;
    packed_t p;
    unpacked_t u;
    logic bit_value;
    logic [3:0] part_value;
    logic [7:0] row_value;
    int i, calls;

    function automatic int pick(input int value);
        calls++;
        return value;
    endfunction

    initial begin
        calls = 0;
        i = 1;
        p = tagged B (8'hff);
        bit_value = p.A[3];
        part_value = p.A[i +: 4];
        p.A[2] = 1'b0;
        p.A[pick(4) +: 2] = 2'b00;
        p.A[i] <= 1'b0;
        u = tagged Count (7);
        row_value = u.Row[i];
        u.Row[pick(0)] = 8'h00;
        #1;
        $fdisplay(STDERR, "bit=%b part=%b row=%h calls=%0d", bit_value, part_value,
                  row_value, calls);
        $fdisplay(STDERR, "p B=%h u Count=%0d", p.B, u.Count);
        $finish;
    end
endmodule

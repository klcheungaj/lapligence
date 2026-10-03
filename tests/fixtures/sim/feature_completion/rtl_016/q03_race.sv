// IEEE 1800-2009 4.6, 4.7, 4.9.4, 10.4.2, 11.9: processes woken by the same
// event run in an unspecified order. `blocking_target` is retagged by a
// blocking write in another process: either the retag precedes the member
// NBA's issue (an issue-time error) or follows it (a commit-time error, since
// every Active-region write precedes the NBA region). `queued_target` races
// a member NBA with a whole-variable NBA: the member write either commits
// first or finds tag B at commit. Both orders end with tag B and payload 55.
typedef union tagged packed {
    logic [7:0] A;
    logic [7:0] B;
} item_t;

module tb;
    localparam logic [31:0] STDERR = 32'h8000_0002;
    item_t blocking_target, queued_target;
    event go;

    always @(go) blocking_target.A <= 8'h77;
    always @(go) blocking_target = tagged B (8'h55);
    always @(go) queued_target.A <= 8'h66;
    always @(go) queued_target <= tagged B (8'h55);

    initial begin
        blocking_target = tagged A (8'h01);
        queued_target = tagged A (8'h02);
        #1 -> go;
        #1;
        if (blocking_target matches tagged B .b)
            $fdisplay(STDERR, "blocking_target B=%h", b);
        if (queued_target matches tagged B .q)
            $fdisplay(STDERR, "queued_target B=%h", q);
        $finish;
    end
endmodule

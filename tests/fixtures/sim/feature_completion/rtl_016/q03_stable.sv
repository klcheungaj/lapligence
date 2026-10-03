// IEEE 1800-2009 4.9.4, 10.4.2, 11.9: a member NBA fixes its target and
// value at issue and performs the member assignment at commit, where it is
// checked against the tag current then. Every commit below finds the issued
// member active, so each write is published.
typedef union tagged packed {
    void Stop;
    logic [7:0] A;
    logic [7:0] B;
} item_t;

typedef union tagged packed {
    logic [3:0] P;
    logic [3:0] Q;
} inner_t;

typedef union tagged packed {
    void Halt;
    inner_t Data;
} outer_t;

typedef struct { logic [3:0] lo; bit [7:0] code; } record_t;

typedef union tagged {
    int Count;
    record_t Record;
} wide_t;

module tb;
    item_t round_trip, member_then_whole, delayed, last_wins;
    item_t cells [0:1];
    outer_t nested;
    wide_t wide;
    logic [7:0] seed;
    int index;

    initial begin
        round_trip = tagged A (8'h01);
        member_then_whole = tagged A (8'h02);
        delayed = tagged A (8'h03);
        last_wins = tagged A (8'h04);
        cells[0] = tagged A (8'h05);
        cells[1] = tagged A (8'h06);
        nested = tagged Data (tagged P (4'h1));
        wide = tagged Record '{lo: 4'h2, code: 8'h20};
        seed = 8'h40;
        index = 1;
        #1;
        // Retagging away and back before commit leaves A active again.
        round_trip.A <= seed + 8'h04;
        round_trip = tagged B (8'h99);
        round_trip = tagged A (8'h11);
        // The member NBA commits first, then the whole NBA retags.
        member_then_whole.A <= 8'h77;
        member_then_whole <= tagged B (8'h55);
        // A future member NBA keeps its issue-time value.
        delayed.A <= #2 seed;
        seed = 8'h00;
        // Same-target member NBAs commit in issue order.
        last_wins.A <= 8'h31;
        last_wins.A <= 8'h32;
        // The selected element is fixed at issue.
        cells[index].A <= 8'h66;
        index = 0;
        cells[1] = tagged A (8'h16);
        // Both nested guards stay valid.
        nested.Data.P <= 4'h7;
        nested = tagged Data (tagged P (4'h2));
        // Unpacked tagged member NBA.
        wide.Record.code <= 8'h21;
        wide.Record.lo = 4'h3;
        #1;
        $display("round_trip A=%h", round_trip.A);
        if (member_then_whole matches tagged B .b)
            $display("member_then_whole B=%h", b);
        $display("delayed before=%h", delayed.A);
        $display("last_wins A=%h", last_wins.A);
        $display("cells A=%h %h", cells[0].A, cells[1].A);
        $display("nested P=%h", nested.Data.P);
        $display("wide lo=%h code=%h", wide.Record.lo, wide.Record.code);
        #2;
        $display("delayed after=%h", delayed.A);
        $finish(0);
    end
endmodule

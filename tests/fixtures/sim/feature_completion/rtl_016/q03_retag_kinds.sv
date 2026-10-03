// IEEE 1800-2009 4.9.4, 10.4.2, 7.3.2, 11.9: within one process the order of
// issue, blocking retag and commit is deterministic. A member NBA performs a
// member assignment at commit; when another member (including void, a
// narrower member or another nested tag) is active then, the assignment is a
// run-time error and the queued payload is not stored under the new tag.
// Results go to stderr so each line keeps its order relative to the reports.
typedef union tagged packed {
    void V;
    logic [7:0] A;
    logic [7:0] B;
    logic [3:0] C;
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
    localparam logic [31:0] STDERR = 32'h8000_0002;
    item_t other, to_void, narrower, whole_first, delayed;
    item_t cells [0:1];
    outer_t inner_retag, outer_retag;
    wide_t wide;
    int index;

    initial begin
        other = tagged A (8'h01);
        to_void = tagged A (8'h02);
        narrower = tagged A (8'h03);
        whole_first = tagged A (8'h04);
        delayed = tagged A (8'h05);
        cells[0] = tagged A (8'h06);
        cells[1] = tagged A (8'h07);
        inner_retag = tagged Data (tagged P (4'h1));
        outer_retag = tagged Data (tagged P (4'h2));
        wide = tagged Record '{lo: 4'h1, code: 8'h10};
        index = 1;
        #1;
        other.A <= 8'h71;
        other = tagged B (8'h52);
        to_void.A <= 8'h72;
        to_void = tagged V;
        narrower.A <= 8'h73;
        narrower = tagged C (4'h3);
        whole_first <= tagged B (8'h54);
        whole_first.A <= 8'h74;
        cells[index].A <= 8'h77;
        index = 0;
        cells[1] = tagged B (8'h17);
        inner_retag.Data.P <= 4'h7;
        inner_retag = tagged Data (tagged Q (4'h8));
        outer_retag.Data.P <= 4'h9;
        outer_retag = tagged Halt;
        wide.Record.code <= 8'h7e;
        wide = tagged Count (-2);
        delayed.A <= #2 8'h75;
        #1;
        if (other matches tagged B .b) $fdisplay(STDERR, "other B=%h", b);
        if (to_void matches tagged V) $fdisplay(STDERR, "to_void V");
        if (narrower matches tagged C .c) $fdisplay(STDERR, "narrower C=%h", c);
        if (whole_first matches tagged B .w) $fdisplay(STDERR, "whole_first B=%h", w);
        $fdisplay(STDERR, "cells A=%h B=%h", cells[0].A, cells[1].B);
        $fdisplay(STDERR, "inner Q=%h", inner_retag.Data.Q);
        if (outer_retag matches tagged Halt) $fdisplay(STDERR, "outer Halt");
        $fdisplay(STDERR, "wide Count=%0d", wide.Count);
        delayed = tagged B (8'h15);
        #2;
        $fdisplay(STDERR, "delayed B=%h", delayed.B);
        $finish;
    end
endmodule

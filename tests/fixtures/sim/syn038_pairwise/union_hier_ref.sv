// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/union_hier_ref.sv
// IEEE 1800-2009 §§6.21, 7.3.1, 13.5.2, and 23.8: a hierarchical task call
// passes a whole untagged packed union by reference; the task writes one named
// packed-struct field, and a local call provides a same-task alias control.
package union_hier_ref_pkg;
    typedef struct packed {
        logic [7:0] high;
        logic [7:0] low;
    } halves_t;

    typedef union packed {
        logic [15:0] word;
        halves_t halves;
    } payload_t;
endpackage

module union_writer(output logic [15:0] local_result);
    import union_hier_ref_pkg::*;

    payload_t local_value;

    task automatic update(ref payload_t target);
        target.halves.low = target.halves.low + 8'h01;
    endtask

    initial begin
        local_value.word = 16'hab40;
        update(local_value);
        if (local_value.word !== 16'hab41)
            $fatal(1, "local union ref task did not update its actual");
        local_result = local_value.word;
    end
endmodule

module tb;
    import union_hier_ref_pkg::*;

    payload_t u;
    logic [15:0] local_result;

    union_writer c0(.local_result(local_result));

    initial begin
        u.word = 16'h12fe;
        c0.update(u);
        #1;
        if (u.word !== 16'h12ff || local_result !== 16'hab41)
            $fatal(1, "hierarchical union ref task produced wrong values");
        $display("hier=%h child-local=%h view=%h/%h",
                 u.word, local_result, u.halves.high, u.halves.low);
        $finish(0);
    end
endmodule

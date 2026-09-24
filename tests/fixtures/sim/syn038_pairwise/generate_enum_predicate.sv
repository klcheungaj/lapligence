// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/generate_enum_predicate.sv
// IEEE 1800-2009 §§6.19, 9.4, 23.3, and 27.4: an enum
// equality is used in a generated event control and as a generated child-port
// actual; the one-bit predicate is distinct from its enum source.
package generate_enum_predicate_pkg;
    typedef enum logic [1:0] {
        IDLE = 2'b00,
        ACTIVE = 2'b01
    } phase_t;
endpackage

module predicate_child(input logic predicate, output wire seen);
    assign seen = predicate;
endmodule

module tb;
    import generate_enum_predicate_pkg::*;

    phase_t phase;
    bit armed;
    int event_count = 0;
    logic [0:0] child_seen;

    for (genvar g = 0; g < 1; g++) begin : generated
        always @(phase == ACTIVE) begin
            if (armed)
                event_count = event_count + 1;
        end

        predicate_child u_predicate(
            .predicate(phase == ACTIVE),
            .seen(child_seen[g])
        );
    end

    initial begin
        phase = IDLE;
        #1;
        armed = 1'b1;
        phase = ACTIVE;
        #1;
        if (event_count != 1 || child_seen[0] !== 1'b1)
            $fatal(1, "generated enum equality paths did not settle once");
        $display("events=%0d", event_count);
        $display("seen=%b", child_seen[0]);
        $finish(0);
    end
endmodule

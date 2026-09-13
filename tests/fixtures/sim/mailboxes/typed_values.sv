// llg-test-fixture: tests/fixtures/sim/mailboxes/typed_values.sv
// IEEE 1800-2009 §15.4 and Annex G.4: packed four-state conversion, real and
// shortreal values, typedef/enum element types, and writable try destinations.
typedef logic [7:0] byte_t;
typedef enum logic [1:0] {ZERO, ONE} state_t;

module tb;
    mailbox #(byte_t) bytes = new(1);
    mailbox #(state_t) states = new(1);
    mailbox #(real) reals = new(1);
    mailbox #(shortreal) shorts = new(1);
    byte_t byte_value;
    state_t state_value;
    real real_value;
    shortreal short_value;

    initial begin
        bytes.put(8'hx5);
        $display("byte_try=%0d", bytes.try_peek(byte_value));
        $display("byte=%h n=%0d", byte_value, bytes.num());
        $display("byte_get=%0d", bytes.try_get(byte_value));

        states.put(ONE);
        states.get(state_value);
        $display("state=%0d n=%0d", state_value, states.num());

        reals.put(1.25);
        reals.peek(real_value);
        $display("real=%f n=%0d", real_value, reals.num());

        shorts.put(1.23456789);
        shorts.get(short_value);
        $display("short=%f n=%0d", short_value, shorts.num());
        $finish;
    end
endmodule

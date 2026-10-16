// Decision S99-D6 (llg policy, an llg limit): llg runs its built-in system
// tasks and functions natively and does not let a VPI registration override
// them. When an application registers the name of a built-in the design
// calls, the run stops at startup naming the built-in and its call; a
// registration of a built-in the design does not call is accepted.
//
// IEEE 1800-2009 36.3.2 (SystemVerilog-1800-2009.txt L56427-56431): "If a
// user-provided PLI application is associated with the same name as a
// built-in system task or system function (using the PLI mechanism), the
// user-provided C application shall override the built-in system task or
// system function, replacing its functionality with that of the
// user-provided C application. For example, a user could write a random
// number generator as a PLI application and then associate the application
// with the name $random, thereby overriding the built-in $random function
// with the user's application."
// Load S99-D6_builtin_override.c as a VPI application. A simulator that
// supports the override prints "random 4"; llg prints nothing and exits
// with status 1, reporting that it does not support overriding `$random`.
module tb;
  integer r;
  initial begin
    r = $random;
    $display("random %0d", r);
    $finish;
  end
endmodule

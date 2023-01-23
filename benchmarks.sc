#!/usr/bin/env amm

import ammonite.ops._
import java.lang.{Process, ProcessBuilder}
import scala.collection.JavaConverters._
import $file.build, build.{relps, relp, binp, format}

type AddressArg = String;
type LocalRunner = (AddressArg) => Runner;
type RemoteRunner = (AddressArg, AddressArg, Int) => Runner;
type ClientRunner = (AddressArg, AddressArg) => Runner;

case class BenchmarkImpl(
	symbol: String, 
	label: String, 
	local: LocalRunner, 
	remote: RemoteRunner, 
	client: ClientRunner,
	mustCopy: List[Path]) {
	def localRunner(benchRunnerAddr: AddressArg): BenchmarkRunner =
		BenchmarkRunner(info, local(benchRunnerAddr));
	def remoteRunner(benchRunnerAddr: AddressArg, benchMasterAddr: AddressArg, numClients: Int): BenchmarkRunner =
		BenchmarkRunner(info, remote(benchRunnerAddr, benchMasterAddr, numClients));
	def clientRunner(benchMasterAddr: AddressArg, benchClientAddr: AddressArg): BenchmarkRunner =
		BenchmarkRunner(info, client(benchMasterAddr, benchClientAddr));
	def info: BenchmarkInfo = BenchmarkInfo(symbol, label);
}

case class BenchmarkInfo(symbol: String, label: String)

case class Runner(env: Path, exec: Path, args: Seq[Shellable])

case class BenchmarkRunner(bench: BenchmarkInfo, runner: Runner) {
	def symbol: String = bench.symbol;
	def label: String = bench.label;
	def run(logFolder: Path): Process = {
		var command = (runner.exec.toString +: runner.args.flatMap(_.s) :+ logFolder.last.toString).toList.asJava;
		val pb = new ProcessBuilder(command);
		val env = pb.environment();
		//env.put("RUST_BACKTRACE", "1"); // TODO remove this for non-testing!
		//env.put("JAVA_OPTS", "-Xms1G -Xmx32G -XX:+UseG1GC");
		pb.directory(runner.env.toIO);
		pb.redirectError(ProcessBuilder.Redirect.appendTo(errorLog(logFolder)));
		pb.redirectOutput(ProcessBuilder.Redirect.appendTo(outputLog(logFolder)));
		val childProcess = pb.start();
		val closeChildThread = new Thread() {
		    override def run(): Unit = {
		        childProcess.destroy();
		    }
		};
		Runtime.getRuntime().addShutdownHook(closeChildThread);
		childProcess
	}
	lazy val fileLabel: String = bench.label.toLowerCase().replaceAll(" ", "_");
	def outputLog(logFolder: Path) = (logFolder / s"${fileLabel}.out").toIO;
	def errorLog(logFolder: Path) = (logFolder / s"${fileLabel}.error").toIO;
}

val javaBin = binp('java);
//val javaOpts = Seq[Shellable]("-Xms1G", "-Xmx32G", "-XX:+UseG1GC","-XX:+HeapDumpOnOutOfMemoryError");
val javaOpts = Seq[Shellable]("-Xms1G", "-Xmx8G", "-XX:+UseG1GC","-XX:+HeapDumpOnOutOfMemoryError");

val implementations: Map[String, BenchmarkImpl] = Map(
	"KOMPACTMIX" -> BenchmarkImpl(
		symbol="KOMPACTMIX",
		label="Kompact Mixed",
		local = (benchRunnerAddr) => Runner(relp("kompact"), relp("kompact/target/release/kompact_benchmarks"), Seq("mixed", benchRunnerAddr)),
		remote = (benchRunnerAddr, benchMasterAddr, numClients) => Runner(relp("kompact"), relp("kompact/target/release/kompact_benchmarks"), Seq("mixed", benchRunnerAddr, benchMasterAddr, numClients)),
		client = (benchMasterAddr, benchClientAddr) => Runner(relp("kompact"), relp("kompact/target/release/kompact_benchmarks"), Seq("mixed", benchMasterAddr, benchClientAddr)),
		mustCopy = List(relp("kompact/target/release/kompact_benchmarks"), relp("kompact/configs"))
	),
);

implicit class AddressArgImpl(arg: AddressArg) {
	private lazy val (first, second) = {
		val split = arg.split(":");
		assert(split.length == 2);
		val addr = split(0);
		val port = split(1).toInt;
		(addr, port)
	};

	def address: String = first;
	def port: Int = second;
}

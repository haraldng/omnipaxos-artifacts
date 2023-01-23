package se.kth.benchmarks

import org.scalatest._
import scala.util.{Failure, Success, Try}
import scala.concurrent.{Await, ExecutionContext, Future}
import scala.concurrent.duration._
import kompics.benchmarks.benchmarks._
import kompics.benchmarks.messages._
import kompics.benchmarks.distributed._
import io.grpc.{ManagedChannelBuilder, Server, ServerBuilder}
import java.util.concurrent.Executors

class LocalTest extends FunSuite with Matchers {
  test("Local communication") {
    val ltest = new se.kth.benchmarks.test.LocalTest(TestRunner);
    ltest.test();
  }

  test("Local failures") {
    for (s <- Stage.list) {
      val ltest = new se.kth.benchmarks.test.LocalTest(new FailRunner(s));
      ltest.testFail();
    }
  }
}

class FailRunner(s: Stage) extends BenchmarkRunnerGrpc.BenchmarkRunner {
  implicit val futurePool = ExecutionContext.fromExecutor(Executors.newSingleThreadExecutor());

  override def ready(request: ReadyRequest): Future[ReadyResponse] = {
    Future.successful(ReadyResponse(true))
  }
  override def shutdown(request: ShutdownRequest): Future[ShutdownAck] = {
    ???
  }
}

object TestRunner extends BenchmarkRunnerGrpc.BenchmarkRunner {
  implicit val futurePool = ExecutionContext.fromExecutor(Executors.newSingleThreadExecutor());

  override def ready(request: ReadyRequest): Future[ReadyResponse] = {
    Future.successful(ReadyResponse(true))
  }
  override def shutdown(request: ShutdownRequest): Future[ShutdownAck] = {
    ???
  }
}

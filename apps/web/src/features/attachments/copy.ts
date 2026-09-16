import type { Locale } from '../../platform/localization/locale';
import { createTranslator } from '../../platform/localization/runtime';
export function attachmentCopy(locale:Locale){
 const t=createTranslator(locale);
 return{title:t('attachments:title'),drop:t('attachments:drop'),choose:t('attachments:choose'),limit:t('attachments:limit'),unavailable:t('attachments:unavailable'),empty:t('attachments:empty'),refresh:t('attachments:refresh'),clear:t('attachments:clear'),cancel:t('attachments:cancel'),retry:t('attachments:retry'),download:t('attachments:download'),remove:t('attachments:remove'),confirm:t('attachments:confirm'),keep:t('attachments:keep'),loading:t('attachments:loading'),hashMissing:t('attachments:hashMissing'),identity:t('attachments:identity'),downloading:t('attachments:downloading'),status:{queued:t('attachments:status.queued'),uploading:t('attachments:status.uploading'),committing:t('attachments:status.committing'),succeeded:t('attachments:status.succeeded'),error:t('attachments:status.error'),canceled:t('attachments:status.canceled')}};
}
export function attachmentProblem(locale:Locale,code:string):string{
 const t=createTranslator(locale);
 switch(code){
  case'file.too_large':return t('attachments:problem.tooLarge');case'file.name_invalid':return t('attachments:problem.nameInvalid');case'file.name_long':return t('attachments:problem.nameLong');case'file.queue_closed':return t('attachments:problem.queueClosed');case'file.queue_full':return t('attachments:problem.queueFull');case'file.owner_invalid':return t('attachments:problem.ownerInvalid');case'file.id_invalid':return t('attachments:problem.idInvalid');case'file.not_ready':return t('attachments:problem.notReady');case'file.cancel_unknown':return t('attachments:problem.cancelUnknown');case'file.cancel_unsent':return t('attachments:problem.cancelUnsent');case'file.invalid_receipt':return t('attachments:problem.receiptMismatch');case'file.refresh_committed':return t('attachments:problem.refreshCommitted');case'file.unlink_failed':return t('attachments:problem.unlinkFailed');case'file.integrity_failed':case'file.download_incomplete':return t('attachments:problem.integrity');default:return t('attachments:problem.unknown');
 }
}
